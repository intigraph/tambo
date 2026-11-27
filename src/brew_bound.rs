//! Bounded Homebrew steps (aegis-f0ne).
//!
//! Measured on the Mac on 2026-11-27: every `/usr/bin/mdfind` started from a
//! launchd agent hangs. Brew runs one for every stack formula it installs. The
//! formulas have no bottle, so `FormulaInstaller#install` runs the
//! build-from-source checks, which ask Spotlight for Xcode.app when only the
//! Command Line Tools are present. By then `brew upgrade` has already unlinked
//! the previous keg, so the tool vanished from PATH for as long as the hang
//! lasted. The consumer's own deadline then killed everything and left the keg
//! unlinked.
//!
//! Every brew step therefore runs under a bound. On timeout Tambo kills brew's
//! whole descendant tree, found through `ps`, because brew's `mdfind` runs in
//! a process group of its own and survived a group kill as an orphan. Brew
//! itself stays in Tambo's process group, so a supervisor that group-kills
//! Tambo also kills brew. The caller then restores the keg's link and names the
//! member.
//!
//! Two bounds apply. Each step gets at most `TAMBO_BREW_STEP_TIMEOUT_SECS`
//! (240s). All ordinary steps of one Tambo process share
//! `TAMBO_BREW_BUDGET_SECS` (600s), which stays under the Mac consumer's 900s
//! member budget. Link repair after a failure is not charged to that budget.
//! Each repair step gets 30s, so a spent budget still leaves the keg linked.

use std::{
    ffi::OsStr,
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

/// Default bound for one brew step. A normal upgrade of a stack formula takes
/// seconds; the Mac consumer gives the whole member update 900s.
pub const DEFAULT_STEP_TIMEOUT: Duration = Duration::from_secs(240);

/// Default total for all ordinary brew steps of one Tambo process.
pub const DEFAULT_BUDGET: Duration = Duration::from_secs(600);

/// Bound for each link-repair step after a failure.
pub const REPAIR_TIMEOUT: Duration = Duration::from_secs(30);

fn seconds(name: &str, default: Duration) -> Duration {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .map_or(default, Duration::from_secs)
}

/// The per-step bound, overridable for tests and slow hosts.
#[must_use]
pub fn step_timeout() -> Duration {
    seconds("TAMBO_BREW_STEP_TIMEOUT_SECS", DEFAULT_STEP_TIMEOUT)
}

/// The shared budget for this process's ordinary brew steps.
fn budget() -> Duration {
    seconds("TAMBO_BREW_BUDGET_SECS", DEFAULT_BUDGET)
}

/// When this process's brew budget ends. The clock starts at the first brew step.
fn budget_end() -> Instant {
    static END: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *END.get_or_init(|| Instant::now() + budget())
}

/// Which bound a step runs under.
#[derive(Clone, Copy)]
pub enum Bound {
    /// An ordinary step: the step bound, capped by what is left of the budget.
    Step,
    /// Link repair after a failure: a fixed short bound outside the budget.
    Repair,
}

impl Bound {
    fn timeout(self) -> Duration {
        match self {
            Bound::Step => {
                step_timeout().min(budget_end().saturating_duration_since(Instant::now()))
            }
            Bound::Repair => REPAIR_TIMEOUT,
        }
    }
}

/// A brew step that did not finish within its bound, or never started
/// because the shared budget was already spent.
#[derive(Debug)]
pub struct TimedOut {
    pub elapsed: Duration,
    /// Processes killed, the brew process included.
    pub killed: usize,
    /// The step was not started: the budget had run out (eph-review-07aef5).
    pub budget_spent: bool,
}

impl std::fmt::Display for TimedOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.budget_spent {
            // Keeps the `timed out after <N>s ` shape the Mac consumer parses.
            return write!(
                f,
                "timed out after {}s (the shared brew budget TAMBO_BREW_BUDGET_SECS \
                 was spent before this step started; nothing was run)",
                self.elapsed.as_secs()
            );
        }
        write!(
            f,
            "timed out after {}s (killed {} process{})",
            self.elapsed.as_secs(),
            self.killed,
            if self.killed == 1 { "" } else { "es" }
        )
    }
}

impl std::error::Error for TimedOut {}

/// What a bounded step produced.
#[derive(Debug)]
pub struct Finished {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
}

/// How a bounded step treats its standard output.
#[derive(Clone, Copy)]
pub enum Output {
    /// Passed through to Tambo's own output (install/upgrade progress).
    Inherit,
    /// Captured and returned (JSON reads).
    Capture,
}

/// Run `program args` with `HOMEBREW_NO_AUTO_UPDATE=1` under `bound`.
pub fn run<I, S>(
    program: &OsStr,
    args: I,
    output: Output,
    bound: Bound,
) -> anyhow::Result<Result<Finished, TimedOut>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let timeout = bound.timeout();
    if timeout.is_zero() {
        return Ok(Err(TimedOut {
            elapsed: budget(),
            killed: 0,
            budget_spent: true,
        }));
    }
    run_for(program, args, output, timeout)
}

fn run_for<I, S>(
    program: &OsStr,
    args: I,
    output: Output,
    timeout: Duration,
) -> anyhow::Result<Result<Finished, TimedOut>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    use std::io::{Read, Seek};
    let mut command = Command::new(program);
    command
        .args(args)
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        // Brew stays in Tambo's process group (eph-review-1d484a): a
        // supervisor that group-kills Tambo must take brew with it.
        .stdin(Stdio::null());
    // A file, not a pipe: a full pipe would block brew and look like a hang.
    let mut captured = match output {
        Output::Capture => {
            let file = tempfile::tempfile()?;
            command.stdout(file.try_clone()?);
            Some(file)
        }
        Output::Inherit => None,
    };
    let start = Instant::now();
    let mut child = command.spawn()?;
    // Descendants seen while the step runs, with their groups, so a process
    // whose parent exits before a timeout is still found (eph-review-07aef5).
    let mut known: Vec<(u32, u32)> = Vec::new();
    let mut polls: u32 = 0;
    loop {
        if let Some(status) = child.try_wait()? {
            let mut stdout = Vec::new();
            if let Some(file) = captured.as_mut() {
                file.rewind()?;
                file.read_to_end(&mut stdout)?;
            }
            return Ok(Ok(Finished { status, stdout }));
        }
        if start.elapsed() >= timeout {
            let killed = kill_tree(child.id(), &known);
            let _ = child.wait();
            return Ok(Err(TimedOut {
                elapsed: start.elapsed(),
                killed,
                budget_spent: false,
            }));
        }
        polls += 1;
        if polls % 10 == 0 {
            remember_descendants(child.id(), &process_table(), &mut known);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Add `root`'s current descendants, with their process groups, to `known`.
fn remember_descendants(root: u32, table: &[Row], known: &mut Vec<(u32, u32)>) {
    let mut frontier = children_of(table, root);
    while let Some(pid) = frontier.pop() {
        if let Some(row) = table.iter().find(|row| row.pid == pid) {
            if !known.contains(&(row.pid, row.pgid)) {
                known.push((row.pid, row.pgid));
            }
        }
        frontier.extend(children_of(table, pid));
    }
}

/// SIGKILL `root` and every descendant, in any process group. Never signals
/// a process group: brew shares Tambo's. Returns how many processes were
/// signalled.
///
/// A descendant whose parent already exited has been reparented to init and
/// is no longer anyone's child (eph-review-07aef5). Two rules still find it:
/// it was seen as a descendant earlier (`known`, matched on pid AND group so
/// a reused pid is not killed) and now has parent 1, or it shares the
/// process group of a live descendant, other than Tambo's own group.
fn kill_tree(root: u32, known: &[(u32, u32)]) -> usize {
    // SAFETY: getpgrp only reads this process's group id.
    let ours = u32::try_from(unsafe { libc::getpgrp() }).unwrap_or(0);
    // Freeze the tree first so nothing forks a new child between the listing
    // and the kill. A stopped process cannot fork.
    let mut seen = Vec::new();
    let mut frontier = vec![root];
    loop {
        let table = process_table();
        while let Some(pid) = frontier.pop() {
            if seen.contains(&pid) {
                continue;
            }
            signal(pid, libc::SIGSTOP);
            seen.push(pid);
            frontier.extend(children_of(&table, pid));
        }
        // Earlier descendants, now reparented to init.
        frontier.extend(
            table
                .iter()
                .filter(|row| {
                    row.ppid == 1
                        && known.contains(&(row.pid, row.pgid))
                        && !seen.contains(&row.pid)
                })
                .map(|row| row.pid),
        );
        // Processes in a descendant's own group that are not yet seen.
        let groups: Vec<u32> = table
            .iter()
            .filter(|row| seen.contains(&row.pid) && row.pgid != ours)
            .map(|row| row.pgid)
            .collect();
        frontier.extend(
            table
                .iter()
                .filter(|row| groups.contains(&row.pgid) && !seen.contains(&row.pid))
                .map(|row| row.pid),
        );
        if frontier.is_empty() {
            break;
        }
    }
    for pid in &seen {
        signal(*pid, libc::SIGKILL);
    }
    seen.len()
}

fn signal(pid: u32, sig: libc::c_int) {
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        // SAFETY: kill only sends a signal; a vanished pid is ESRCH.
        unsafe {
            libc::kill(pid, sig);
        }
    }
}

struct Row {
    pid: u32,
    ppid: u32,
    pgid: u32,
}

/// Every process with its parent and group, from `ps` (portable across macOS
/// and Linux; `/proc` is absent on the Mac).
fn process_table() -> Vec<Row> {
    let Ok(out) = Command::new("/bin/ps")
        .args(["-A", "-o", "pid=,ppid=,pgid="])
        .output()
    else {
        return Vec::new();
    };
    parse_table(&String::from_utf8_lossy(&out.stdout))
}

fn parse_table(listing: &str) -> Vec<Row> {
    listing
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some(Row {
                pid: fields.next()?.parse().ok()?,
                ppid: fields.next()?.parse().ok()?,
                pgid: fields.next()?.parse().ok()?,
            })
        })
        .collect()
}

fn children_of(table: &[Row], parent: u32) -> Vec<u32> {
    table
        .iter()
        .filter(|row| row.ppid == parent)
        .map(|row| row.pid)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn children_are_read_from_a_ps_listing() {
        let listing =
            "  1     0  1\n 10     1 10\n 11    10 10\n 12    10 12\n garbage\n 13 10 10\n";
        let table = parse_table(listing);
        assert_eq!(children_of(&table, 10), vec![11, 12, 13]);
        assert!(children_of(&table, 99).is_empty());
        assert_eq!(table.iter().find(|r| r.pid == 12).unwrap().pgid, 12);
    }

    #[test]
    fn a_reparented_descendant_in_a_child_group_is_killed() {
        // eph-review-07aef5: the parent of an own-group process exits first,
        // so the process is reparented to init and is nobody's child.
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join("orphan.pid");
        // The middle process starts its own group, forks a sleeper into it,
        // records it and exits after 1s; the outer shell then hangs.
        let script = format!(
            "perl -e 'setpgrp(0,0); my $p = fork(); if ($p == 0) {{ exec \"sleep\", \"300\" }} \
             open(F, \">\", \"{}\"); print F $p; close F; sleep 1; exit 0'; sleep 300",
            pidfile.display()
        );
        let timed_out = run_for(
            OsStr::new("/bin/sh"),
            ["-c", &script],
            Output::Inherit,
            Duration::from_secs(3),
        )
        .unwrap()
        .unwrap_err();
        let orphan: libc::pid_t = std::fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let mut alive = true;
        for _ in 0..40 {
            // SAFETY: signal 0 only probes for existence.
            alive = unsafe { libc::kill(orphan, 0) } == 0;
            if !alive {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!alive, "reparented orphan survived: {timed_out}");
    }

    #[test]
    fn a_spent_budget_is_reported_as_such() {
        let text = TimedOut {
            elapsed: Duration::from_secs(600),
            killed: 0,
            budget_spent: true,
        }
        .to_string();
        assert!(text.starts_with("timed out after 600s ("), "{text}");
        assert!(text.contains("budget"), "{text}");
        assert!(!text.contains("killed"), "{text}");
    }

    #[test]
    fn brew_stays_in_tambos_process_group() {
        // eph-review-1d484a: a supervisor that group-kills Tambo must take
        // brew with it, so a step must not start a process group of its own.
        let done = run_for(
            OsStr::new("/bin/sh"),
            ["-c", "ps -o pgid= -p $$"],
            Output::Capture,
            Duration::from_secs(10),
        )
        .unwrap()
        .unwrap();
        // SAFETY: getpgrp only reads this process's group id.
        let ours = unsafe { libc::getpgrp() };
        assert_eq!(
            String::from_utf8_lossy(&done.stdout).trim(),
            ours.to_string()
        );
    }

    #[test]
    fn a_finished_step_returns_its_captured_output() {
        let done = run_for(
            OsStr::new("/bin/sh"),
            ["-c", "echo hello; exit 3"],
            Output::Capture,
            Duration::from_secs(10),
        )
        .unwrap()
        .unwrap();
        assert_eq!(done.status.code(), Some(3));
        assert_eq!(done.stdout, b"hello\n");
    }

    #[test]
    fn a_hung_step_and_a_child_in_its_own_group_are_both_killed() {
        // The mdfind shape: a grandchild that leaves brew's process group.
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join("orphan.pid");
        let script = format!(
            "perl -e 'setpgrp(0,0); exec \"sleep\", \"300\"' & echo $! > {}; wait",
            pidfile.display()
        );
        let timed_out = run_for(
            OsStr::new("/bin/sh"),
            ["-c", &script],
            Output::Inherit,
            Duration::from_millis(800),
        )
        .unwrap()
        .unwrap_err();
        assert!(timed_out.killed >= 2, "{timed_out}");
        let orphan: libc::pid_t = std::fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let mut alive = true;
        for _ in 0..40 {
            // SAFETY: signal 0 only probes for existence.
            alive = unsafe { libc::kill(orphan, 0) } == 0;
            if !alive {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!alive, "the own-group grandchild survived the timeout");
        let text = timed_out.to_string();
        assert!(
            text.starts_with("timed out after ") && text.contains("s (killed "),
            "{text}"
        );
    }
}
