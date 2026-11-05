//! `tambo prime`: Tambo's agent primer, then each installed stack tool's
//! `prime --brief` (the stack prime contract, `docs/stack/PRIME-CONTRACT.md`).
//!
//! The static half is `src/primer.md`, embedded at build time; the tambo skill
//! carries a generated copy and the book includes the file, and
//! `tests/prime_sync.rs` fails when either drifts.
//!
//! The live half finds stack tools on `PATH`, checks each one's identity (a
//! same-named foreign `sd`, `st` or `dp` is never run with `prime`), then runs
//! `<program> prime --brief`, all tools in parallel, stdin closed, output
//! capped, inside one 1 s deadline per tool. It never installs, writes or
//! registers anything. A tool whose prime exits non-zero or prints nothing is
//! "no prime yet"; one past the deadline has its process group killed and is
//! reported "timed out". Neither fails the command.

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::adapter::which_all;

#[path = "prime_site.rs"]
pub mod site;

/// The static primer. One source: the skill and the book carry copies of it.
pub const PRIMER: &str = include_str!("primer.md");

/// The `--json` shape version (`schema` field). Bump only on a breaking change.
pub const SCHEMA: &str = "stack-prime/1";

/// Wall-clock budget per tool for the identity check plus `prime --brief`
/// together. Tools run in parallel, so this bounds the whole live part.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1000);

/// Most lines kept from one tool's brief (the contract's `--brief` limit).
pub const BRIEF_LINES: usize = 10;

/// Most bytes read from one tool's output; the rest is discarded.
pub const READ_CAP: u64 = 64 * 1024;

/// Serializes child spawns across probe threads (see `run_bounded`).
static SPAWN_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One stack tool `tambo prime` probes.
pub struct Tool {
    pub name: &'static str,
    /// The one program run. `qp` is never probed: it is only ever an alias.
    pub program: &'static str,
    /// Arguments that make the program identify itself.
    pub identity_argv: &'static [&'static str],
    /// The identity output must contain one of these, or the program is a
    /// different tool with the same name and is never run with `prime`.
    pub identity: &'static [&'static str],
}

/// The stack binaries, in stack-table order (Tambo calls; Rikuy is a site). Member
/// identities match their `members/*.toml` manifests (tested below); generic
/// short names (`sd`, `st`, `dp`) are identified by text a same-named tool
/// (sed replacement, suckless terminal) would not print.
pub const ROSTER: &[Tool] = &[
    Tool {
        name: "quipu",
        program: "quipu",
        identity_argv: &["--help"],
        identity: &["knowledge graph"],
    },
    Tool {
        name: "camayoc",
        program: "camayoc",
        identity_argv: &["--version"],
        identity: &["camayoc"],
    },
    Tool {
        name: "quechua",
        program: "quechua",
        identity_argv: &["--version"],
        identity: &["quechua"],
    },
    Tool {
        name: "chaski",
        program: "chaski",
        identity_argv: &["--help"],
        identity: &["a post-commit and time-driven reactor"],
    },
    Tool {
        name: "bobbin",
        program: "bobbin",
        identity_argv: &["--version"],
        identity: &["bobbin"],
    },
    Tool {
        name: "yupana",
        program: "yupana",
        identity_argv: &["--version"],
        identity: &["yupana"],
    },
    Tool {
        name: "desire-path",
        program: "dp",
        identity_argv: &["--help"],
        identity: &["failed AI tool calls"],
    },
    Tool {
        name: "seeds",
        program: "sd",
        identity_argv: &["--help"],
        identity: &[
            "seeds is the beads-compatible tracker",
            "seeds is the work-item tracker for the quipu stack",
        ],
    },
    Tool {
        name: "shantytown",
        program: "st",
        identity_argv: &["--help"],
        identity: &["sling"],
    },
    Tool {
        name: "shuttle",
        program: "shuttle",
        identity_argv: &["--help"],
        identity: &["The `shuttle` CLI"],
    },
];

/// What probing one tool found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    NotInstalled,
    /// A program with the tool's name that is not the stack tool; never run.
    Foreign,
    Primed(Vec<String>),
    NoPrime,
    TimedOut,
}

impl Outcome {
    fn key(&self) -> &'static str {
        match self {
            Self::NotInstalled => "not-installed",
            Self::Foreign => "foreign",
            Self::Primed(_) => "primed",
            Self::NoPrime => "no-prime",
            Self::TimedOut => "timed-out",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Probe {
    pub tool: &'static str,
    pub program: Option<PathBuf>,
    pub outcome: Outcome,
    pub elapsed_ms: u128,
}

/// `(heading, text)` blocks of the primer; block 0 is the title.
fn split(primer: &str) -> Vec<(&str, &str)> {
    let mut starts = vec![(0usize, "")];
    let mut offset = 0;
    for line in primer.split_inclusive('\n') {
        if let Some(h) = line.strip_prefix("## ") {
            starts.push((offset, h.trim_end()));
        }
        offset += line.len();
    }
    starts
        .iter()
        .enumerate()
        .map(|(i, &(start, heading))| {
            let end = starts.get(i + 1).map_or(primer.len(), |s| s.0);
            (heading, &primer[start..end])
        })
        .collect()
}

/// Section headings of the primer (`## ...`), in order.
pub fn sections(primer: &str) -> Vec<&str> {
    split(primer).into_iter().skip(1).map(|(h, _)| h).collect()
}

/// One section by case-insensitive heading prefix.
pub fn section(primer: &str, wanted: &str) -> Result<String, String> {
    let needle = wanted.trim().to_lowercase();
    split(primer)
        .into_iter()
        .skip(1)
        .find(|(h, _)| h.to_lowercase().starts_with(&needle))
        .map(|(_, body)| body.to_string())
        .ok_or_else(|| {
            format!(
                "unknown primer section: {wanted}. Sections: {}",
                sections(primer).join(", ")
            )
        })
}

/// Kill the child's whole process group, so a grandchild cannot outlive it.
#[cfg(unix)]
fn kill_group(child: &mut std::process::Child) {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    if let Ok(pid) = i32::try_from(child.id()) {
        // SAFETY: plain syscall; the child leads its own group (process_group(0)).
        unsafe {
            kill(-pid, 9);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(unix))]
fn kill_group(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Run `program args`, stdin closed, stdout and stderr each capped at
/// [`READ_CAP`] (returned together, stdout first, when `stderr` is set; else
/// stderr is dropped); kill its process group at `deadline`. `None` means timed
/// out; `Some(Err(()))` means it could not start.
fn run_bounded(
    program: &Path,
    args: &[&str],
    home: Option<&Path>,
    stderr: bool,
    deadline: Instant,
) -> Option<Result<(ExitStatus, Vec<u8>), ()>> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(if stderr {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    if let Some(home) = home {
        cmd.env("HOME", home);
    }
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    // One spawn at a time: where pipes are not created close-on-exec
    // atomically (macOS), a concurrent fork can inherit another probe's pipe
    // and hold it open, stalling that probe's reader until its deadline.
    let spawned = {
        let _guard = SPAWN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        cmd.spawn()
    };
    let Ok(mut child) = spawned else {
        return Some(Err(()));
    };
    // Read on threads: a chatty child must not block on a full pipe.
    let readers: Vec<mpsc::Receiver<Vec<u8>>> = [
        child
            .stdout
            .take()
            .map(|o| Box::new(o) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|e| Box::new(e) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .map(|pipe| {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.take(READ_CAP).read_to_end(&mut buf);
            let _ = tx.send(buf);
        });
        rx
    })
    .collect();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            _ => {
                kill_group(&mut child);
                return None;
            }
        }
    };
    let mut bytes = Vec::new();
    for rx in readers {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining.max(Duration::from_millis(20))) {
            Ok(part) => bytes.extend(part),
            Err(_) => {
                // A background grandchild still holds the pipe, so the group
                // is still alive: kill it rather than leave an orphan.
                kill_group(&mut child);
                return Some(Ok((status, bytes)));
            }
        }
    }
    Some(Ok((status, bytes)))
}

/// Is the program at `path` this stack tool? Probed with a throwaway HOME so
/// identifying a tool never touches the user's config or store.
fn identify(tool: &Tool, path: &Path, deadline: Instant) -> Option<bool> {
    let home = tempfile::tempdir().ok()?;
    let (_, bytes) = match run_bounded(path, tool.identity_argv, Some(home.path()), true, deadline)?
    {
        Ok(out) => out,
        Err(()) => return Some(false),
    };
    let text = String::from_utf8_lossy(&bytes);
    Some(tool.identity.iter().any(|marker| text.contains(marker)))
}

/// Identify, then run `prime --brief`, all inside one deadline.
fn probe_one(tool: &Tool, path: &Path, timeout: Duration) -> Outcome {
    let deadline = Instant::now() + timeout;
    match identify(tool, path, deadline) {
        None => return Outcome::TimedOut,
        Some(false) => return Outcome::Foreign,
        Some(true) => {}
    }
    let (status, bytes) = match run_bounded(path, &["prime", "--brief"], None, false, deadline) {
        None => return Outcome::TimedOut,
        Some(Err(())) => return Outcome::NoPrime,
        Some(Ok(out)) => out,
    };
    let lines: Vec<String> = String::from_utf8_lossy(&bytes)
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    if !status.success() || lines.is_empty() {
        return Outcome::NoPrime;
    }
    Outcome::Primed(lines)
}

/// Probe every roster tool on `path`, in parallel, each bounded by `timeout`.
pub fn probe(path: Option<&OsStr>, timeout: Duration) -> Vec<Probe> {
    let handles: Vec<_> = ROSTER
        .iter()
        .map(|tool| {
            let program = which_all(tool.program, path).into_iter().next();
            thread::spawn(move || {
                let started = Instant::now();
                let outcome = match &program {
                    None => Outcome::NotInstalled,
                    Some(p) => probe_one(tool, p, timeout),
                };
                Probe {
                    tool: tool.name,
                    program,
                    outcome,
                    elapsed_ms: started.elapsed().as_millis(),
                }
            })
        })
        .collect();
    handles
        .into_iter()
        .zip(ROSTER)
        .map(|(h, tool)| {
            h.join().unwrap_or(Probe {
                tool: tool.name,
                program: None,
                outcome: Outcome::NoPrime,
                elapsed_ms: 0,
            })
        })
        .collect()
}

fn names(probes: &[Probe], key: &str) -> Vec<&'static str> {
    probes
        .iter()
        .filter(|p| p.outcome.key() == key)
        .map(|p| p.tool)
        .collect()
}

fn installed_count(probes: &[Probe]) -> usize {
    probes
        .iter()
        .filter(|p| p.outcome != Outcome::NotInstalled)
        .count()
}

/// Lines of one tool's brief, cut to [`BRIEF_LINES`] with a pointer to the rest.
fn capped(tool: &str, lines: &[String]) -> Vec<String> {
    if lines.len() <= BRIEF_LINES {
        return lines.to_vec();
    }
    let mut out = lines[..BRIEF_LINES - 1].to_vec();
    out.push(format!(
        "... {} more lines; run `{tool} prime --brief`",
        lines.len() + 1 - BRIEF_LINES
    ));
    out
}

/// The live stack section as primer-style Markdown.
pub fn render_live(probes: &[Probe]) -> String {
    let mut out = String::from("\n## Live state\n\n");
    out.push_str(&format!(
        "Stack tools on PATH: {} of {}.\n",
        installed_count(probes),
        probes.len()
    ));
    for p in probes {
        let program = p
            .program
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        match &p.outcome {
            Outcome::NotInstalled => {}
            Outcome::Primed(lines) => {
                out.push_str(&format!("\n### {} (`{program}`)\n\n", p.tool));
                for line in capped(p.tool, lines) {
                    out.push_str(&format!("    {line}\n"));
                }
            }
            Outcome::NoPrime => {
                out.push_str(&format!("\n### {} (`{program}`)\n\nno prime yet\n", p.tool))
            }
            Outcome::Foreign => out.push_str(&format!(
                "\n### {} (`{program}`)\n\nnot the stack tool (a different program with that name); not run\n",
                p.tool
            )),
            Outcome::TimedOut => out.push_str(&format!(
                "\n### {} (`{program}`)\n\ntimed out; run `{program} prime` directly\n",
                p.tool
            )),
        }
    }
    let missing = names(probes, "not-installed");
    if !missing.is_empty() {
        out.push_str(&format!("\nNot installed: {}.\n", missing.join(", ")));
    }
    out
}

/// `--brief --static`: the version line, "What Tambo is", the pointer line.
pub fn render_static_brief() -> String {
    let what = section(PRIMER, "what").unwrap_or_default();
    let mut out = format!(
        "tambo {}: installs and proves the Quipu stack\n",
        env!("CARGO_PKG_VERSION")
    );
    for line in what.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("Run `tambo prime` for the full primer.\n");
    out
}

/// The contract's `--brief`: 5 to 10 lines.
pub fn render_brief(probes: &[Probe]) -> String {
    let list = |key| {
        let n = names(probes, key);
        if n.is_empty() {
            "none".to_owned()
        } else {
            n.join(", ")
        }
    };
    let mut out = format!(
        "tambo {}: installs and proves the Quipu stack\n",
        env!("CARGO_PKG_VERSION")
    );
    out.push_str(&format!(
        "Stack tools on PATH: {} of {}\n",
        installed_count(probes),
        probes.len()
    ));
    out.push_str(&format!("Primed: {}\n", list("primed")));
    out.push_str(&format!("No prime yet: {}\n", list("no-prime")));
    let timed_out = names(probes, "timed-out");
    if !timed_out.is_empty() {
        out.push_str(&format!("Timed out: {}\n", timed_out.join(", ")));
    }
    let foreign = names(probes, "foreign");
    if !foreign.is_empty() {
        out.push_str(&format!("Not the stack tool: {}\n", foreign.join(", ")));
    }
    out.push_str(&format!("Not installed: {}\n", list("not-installed")));
    out.push_str("Run `tambo prime` for the full primer.\n");
    out
}

/// The contract's common `--json` shape.
pub fn to_json(primer_text: Option<&str>, probes: Option<&[Probe]>, elapsed_ms: u128) -> Value {
    let mut primer = json!({ "sections": sections(PRIMER) });
    if let Some(text) = primer_text {
        primer["text"] = json!(text);
    }
    let live = match probes {
        None => json!({ "status": "skipped" }),
        Some(probes) => {
            let tools: Vec<Value> = probes
                .iter()
                .map(|p| {
                    let mut v = json!({
                        "tool": p.tool,
                        "status": p.outcome.key(),
                        "program": p.program.as_ref().map(|x| x.display().to_string()),
                        "elapsed_ms": p.elapsed_ms,
                    });
                    if let Outcome::Primed(lines) = &p.outcome {
                        v["brief"] = json!(capped(p.tool, lines));
                    }
                    v
                })
                .collect();
            let partial = probes.iter().any(|p| p.outcome == Outcome::TimedOut);
            json!({
                "status": if partial { "partial" } else { "ok" },
                "elapsed_ms": elapsed_ms,
                "summary": { "tools": tools },
            })
        }
    };
    let status = if matches!(live["status"].as_str(), Some("ok" | "skipped")) {
        "ok"
    } else {
        "degraded"
    };
    json!({
        "schema": SCHEMA,
        "tool": "tambo",
        "version": env!("CARGO_PKG_VERSION"),
        "primer": primer,
        "live": live,
        "status": status,
    })
}

pub struct Options<'a> {
    pub brief: bool,
    pub json: bool,
    pub offline: bool,
    pub section: Option<&'a str>,
    pub timeout: Duration,
    pub rikuy_url: Option<&'a str>,
}

/// Entry point. Returns the text to print, or a usage error.
pub fn run(opts: &Options) -> Result<String, String> {
    let text = match opts.section {
        Some(wanted) => section(PRIMER, wanted)?,
        None => PRIMER.to_owned(),
    };
    let started = Instant::now();
    let path = std::env::var_os("PATH");
    let rikuy_url = opts
        .rikuy_url
        .map(str::to_owned)
        .or_else(|| std::env::var("RIKUY_URL").ok());
    let site_handle = (!opts.offline).then(|| {
        let path = path.clone();
        let timeout = opts.timeout;
        thread::spawn(move || site::probe(rikuy_url.as_deref(), path.as_deref(), timeout))
    });
    let probes = (!opts.offline).then(|| probe(path.as_deref(), opts.timeout));
    let site = site_handle.map(|h| {
        h.join().unwrap_or(site::Site {
            url: None,
            status: "unreadable",
            reachable: None,
            generated_at: None,
            revision: None,
            reason: "reader_failed",
            elapsed_ms: 0,
        })
    });
    let elapsed = started.elapsed().as_millis();
    if opts.json {
        let primer_text = (!opts.brief).then_some(text.as_str());
        let mut value = to_json(primer_text, probes.as_deref(), elapsed);
        if let Some(site) = &site {
            value["live"]["summary"]["sites"] = json!([site.value()]);
            if site.reason != "url_not_configured" && site.status != "ok" {
                // A binary deadline remains partial. Other site failures retain
                // their own category; optional, unconfigured sites do not gate.
                if value["live"]["status"] != "partial" {
                    value["live"]["status"] = json!(site.status);
                }
                value["status"] = json!("degraded");
            }
        }
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        ));
    }
    if opts.brief {
        return Ok(match &probes {
            Some(probes) => {
                let mut brief = render_brief(probes);
                if let Some(site) = &site {
                    let end = "Run `tambo prime` for the full primer.\n";
                    brief = brief.replace(end, &format!("{}\n{end}", site.line()));
                }
                brief
            }
            None => render_static_brief(),
        });
    }
    let mut out = text;
    if let Some(probes) = &probes {
        out.push_str(&render_live(probes));
        if let Some(site) = &site {
            out.push_str(&format!("\n### rikuy (static site)\n\n{}\n", site.line()));
            out.push_str(&format!(
                "Read: {} ({}); live brief state not measured.\n",
                site.status, site.reason
            ));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primer_has_the_contract_sections_in_order() {
        assert!(PRIMER.starts_with("# Tambo agent primer\n"));
        let s = sections(PRIMER);
        let required = [
            "What Tambo is",
            "Agent workflow",
            "Core commands",
            "MCP tools",
            "Next steps",
        ];
        let positions: Vec<usize> = required
            .iter()
            .map(|r| {
                s.iter()
                    .position(|h| h == r)
                    .unwrap_or_else(|| panic!("missing {r}"))
            })
            .collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{s:?}");
        assert_eq!(s.last(), Some(&"Next steps"));
    }

    #[test]
    fn section_matches_a_case_insensitive_prefix() {
        assert!(section(PRIMER, "mcp").unwrap().starts_with("## MCP tools"));
        assert!(section(PRIMER, "nope").unwrap_err().contains("Sections:"));
    }

    #[test]
    fn static_brief_is_five_to_ten_lines() {
        let n = render_static_brief().lines().count();
        assert!((5..=10).contains(&n), "{n} lines");
    }

    #[test]
    fn member_identities_match_their_manifests() {
        for tool in ROSTER {
            let Some(m) = crate::members::get(tool.name) else {
                continue;
            };
            let p = m
                .programs
                .iter()
                .find(|p| p.name == tool.program)
                .unwrap_or_else(|| panic!("{}: no program {}", tool.name, tool.program));
            let known: Vec<&str> = std::iter::once(&p.identity_contains)
                .chain(&p.identity_alternatives)
                .map(String::as_str)
                .collect();
            assert_eq!(
                tool.identity_argv,
                p.identity_argv.as_slice(),
                "{}",
                tool.name
            );
            assert_eq!(tool.identity, known.as_slice(), "{}", tool.name);
        }
    }

    #[test]
    fn roster_matches_the_stack_table() {
        let table = include_str!("../docs/stack-table.md");
        let listed: Vec<&str> = table
            .lines()
            .filter_map(|l| l.strip_prefix("| ["))
            .filter_map(|l| l.split(']').next())
            .filter(|name| !matches!(*name, "tambo" | "rikuy"))
            .collect();
        let roster: Vec<&str> = ROSTER.iter().map(|t| t.name).collect();
        assert_eq!(roster, listed);
    }

    #[test]
    fn roster_programs_agree_with_the_adapters() {
        for tool in ROSTER {
            assert_ne!(tool.program, "qp", "qp is an alias, never probed");
            let Some(name) = crate::model::ToolName::parse(tool.name) else {
                continue;
            };
            let known = crate::adapter::programs(name);
            if !known.is_empty() {
                assert!(known.contains(&tool.program), "{}: {known:?}", tool.name);
            }
        }
    }

    #[test]
    fn long_briefs_are_capped() {
        let lines: Vec<String> = (0..25).map(|i| format!("line {i}")).collect();
        let out = capped("quipu", &lines);
        assert_eq!(out.len(), BRIEF_LINES);
        assert!(out.last().unwrap().contains("16 more lines"));
    }

    #[test]
    fn brief_stays_within_ten_lines() {
        let probes: Vec<Probe> = ROSTER
            .iter()
            .enumerate()
            .map(|(i, tool)| Probe {
                tool: tool.name,
                program: Some(PathBuf::from(format!("/bin/{}", tool.program))),
                outcome: match i % 5 {
                    0 => Outcome::Primed(vec!["x".into()]),
                    1 => Outcome::NoPrime,
                    2 => Outcome::TimedOut,
                    3 => Outcome::Foreign,
                    _ => Outcome::NotInstalled,
                },
                elapsed_ms: 1,
            })
            .collect();
        let n = render_brief(&probes).lines().count();
        assert!((5..=10).contains(&n), "{n} lines");
        let v = to_json(None, Some(&probes), 5);
        assert_eq!(v["live"]["status"], "partial");
        assert_eq!(v["status"], "degraded");
        assert_eq!(v["schema"], SCHEMA);
        assert!(v["primer"].get("text").is_none());
    }
}
