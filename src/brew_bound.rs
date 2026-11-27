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
//! Every brew step therefore runs in its own process group under a bound. On
//! timeout Tambo kills the whole descendant tree, not only the group: brew's
//! `mdfind` runs in a process group of its own and survived a group kill as an
//! orphan. The caller then restores the previous keg's link and names the
//! member.

use std::{
    ffi::OsStr,
    os::unix::process::CommandExt,
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

/// Default bound for one brew step. A normal upgrade of a stack formula takes
/// seconds; the Mac consumer gives the whole member update 900s.
pub const DEFAULT_STEP_TIMEOUT: Duration = Duration::from_secs(240);

/// The step bound, overridable for tests and slow hosts.
#[must_use]
pub fn step_timeout() -> Duration {
    std::env::var("TAMBO_BREW_STEP_TIMEOUT_SECS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .map_or(DEFAULT_STEP_TIMEOUT, Duration::from_secs)
}

/// A brew step that did not finish within its bound.
#[derive(Debug)]
pub struct TimedOut {
    pub elapsed: Duration,
    /// Processes killed, the brew process included.
    pub killed: usize,
}

impl std::fmt::Display for TimedOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

/// Run `program args` with `HOMEBREW_NO_AUTO_UPDATE=1` under `timeout`.
pub fn run<I, S>(
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
        .stdin(Stdio::null())
        // A group of its own, so a timeout can signal brew and its children
        // without signalling Tambo or whoever supervises Tambo.
        .process_group(0);
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
            let killed = kill_tree(child.id());
            let _ = child.wait();
            return Ok(Err(TimedOut {
                elapsed: start.elapsed(),
                killed,
            }));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// SIGKILL `root`, every descendant (in any process group) and `root`'s
/// group. Returns how many distinct processes were signalled.
fn kill_tree(root: u32) -> usize {
    // Freeze the tree first so nothing forks a new child between the listing
    // and the kill. A stopped process cannot fork.
    let mut seen = Vec::new();
    let mut frontier = vec![root];
    while let Some(pid) = frontier.pop() {
        if seen.contains(&pid) {
            continue;
        }
        signal(pid, libc::SIGSTOP);
        seen.push(pid);
        frontier.extend(children(pid));
    }
    for pid in &seen {
        signal(*pid, libc::SIGKILL);
    }
    if let Ok(group) = libc::pid_t::try_from(root) {
        // SAFETY: killpg only sends a signal; a vanished group is ESRCH.
        unsafe {
            libc::killpg(group, libc::SIGKILL);
        }
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

/// Direct children of `pid`, from `ps` (portable across macOS and Linux;
/// `/proc` is absent on the Mac).
fn children(pid: u32) -> Vec<u32> {
    let Ok(out) = Command::new("/bin/ps")
        .args(["-A", "-o", "pid=,ppid="])
        .output()
    else {
        return Vec::new();
    };
    parse_children(&String::from_utf8_lossy(&out.stdout), pid)
}

fn parse_children(listing: &str, parent: u32) -> Vec<u32> {
    listing
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse::<u32>().ok()?;
            let ppid = fields.next()?.parse::<u32>().ok()?;
            (ppid == parent).then_some(pid)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn children_are_read_from_a_ps_listing() {
        let listing = "  1     0\n 10     1\n 11    10\n 12    10\n garbage\n 13 10\n";
        assert_eq!(parse_children(listing, 10), vec![11, 12, 13]);
        assert!(parse_children(listing, 99).is_empty());
    }

    #[test]
    fn a_finished_step_returns_its_captured_output() {
        let done = run(
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
        let timed_out = run(
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
        assert!(timed_out
            .to_string()
            .starts_with("timed out after 0s (killed "));
    }
}
