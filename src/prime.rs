//! `tambo prime`: Tambo's agent primer, then each installed stack tool's
//! `prime --brief` (the stack prime contract, `docs/stack/PRIME-CONTRACT.md`).
//!
//! The static half is `src/primer.md`, embedded at build time; the tambo skill
//! carries a generated copy and the book includes the file, and
//! `tests/prime_sync.rs` fails when either drifts.
//!
//! The live half finds stack tools on `PATH` and runs `<program> prime --brief`
//! for each, in parallel, with stdin closed and a per-tool timeout. It never
//! installs, writes or registers anything. A tool whose prime exits non-zero
//! or prints nothing is "no prime yet"; one past its timeout is killed and
//! reported "timed out". Neither fails the command.

use std::ffi::OsStr;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::adapter::which_all;

/// The static primer. One source: the skill and the book carry copies of it.
pub const PRIMER: &str = include_str!("primer.md");

/// Default wall-clock budget for one tool's `prime --brief`.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

/// Most lines kept from one tool's brief (the contract's `--brief` limit).
pub const BRIEF_LINES: usize = 10;

/// The stack tools `tambo prime` probes, in stack-table order (Tambo itself is
/// the caller, not a probe). The first program found on `PATH` is run.
pub const ROSTER: &[(&str, &[&str])] = &[
    ("quipu", &["quipu", "qp"]),
    ("camayoc", &["camayoc"]),
    ("quechua", &["quechua"]),
    ("chaski", &["chaski"]),
    ("bobbin", &["bobbin"]),
    ("yupana", &["yupana"]),
    ("desire-path", &["dp"]),
    ("seeds", &["sd"]),
    ("shantytown", &["st"]),
    ("rikuy", &["rikuy"]),
    ("shuttle", &["shuttle"]),
];

/// What probing one tool found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    NotInstalled,
    Primed(Vec<String>),
    NoPrime,
    TimedOut,
}

impl Outcome {
    fn key(&self) -> &'static str {
        match self {
            Self::NotInstalled => "not-installed",
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

/// Run `program prime --brief` with stdin closed, killing it after `timeout`.
fn run_brief(program: &std::path::Path, timeout: Duration) -> Outcome {
    let Ok(mut child) = Command::new(program)
        .args(["prime", "--brief"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Outcome::NoPrime;
    };
    // Read on a thread: a chatty child must not block on a full pipe, and a
    // grandchild holding the pipe open must not block us past the deadline.
    let mut stdout = child.stdout.take();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(out) = stdout.as_mut() {
            let _ = out.read_to_end(&mut buf);
        }
        let _ = tx.send(buf);
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Outcome::TimedOut;
            }
        }
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    let bytes = rx
        .recv_timeout(remaining.max(Duration::from_millis(50)))
        .unwrap_or_default();
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
        .map(|&(tool, programs)| {
            let program = programs
                .iter()
                .find_map(|p| which_all(p, path).into_iter().next());
            thread::spawn(move || {
                let started = Instant::now();
                let outcome = match &program {
                    None => Outcome::NotInstalled,
                    Some(p) => run_brief(p, timeout),
                };
                Probe {
                    tool,
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
        .map(|(h, &(tool, _))| {
            h.join().unwrap_or(Probe {
                tool,
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
}

/// Entry point. Returns the text to print, or a usage error.
pub fn run(opts: &Options) -> Result<String, String> {
    let text = match opts.section {
        Some(wanted) => section(PRIMER, wanted)?,
        None => PRIMER.to_owned(),
    };
    let started = Instant::now();
    let path = std::env::var_os("PATH");
    let probes = (!opts.offline).then(|| probe(path.as_deref(), opts.timeout));
    let elapsed = started.elapsed().as_millis();
    if opts.json {
        let primer_text = (!opts.brief).then_some(text.as_str());
        let value = to_json(primer_text, probes.as_deref(), elapsed);
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        ));
    }
    if opts.brief {
        return Ok(match &probes {
            Some(probes) => render_brief(probes),
            None => format!(
                "tambo {}: installs and proves the Quipu stack\nRun `tambo prime` for the full primer.\n",
                env!("CARGO_PKG_VERSION")
            ),
        });
    }
    let mut out = text;
    if let Some(probes) = &probes {
        out.push_str(&render_live(probes));
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
    fn roster_matches_the_stack_table() {
        let table = include_str!("../docs/stack-table.md");
        let listed: Vec<&str> = table
            .lines()
            .filter_map(|l| l.strip_prefix("| ["))
            .filter_map(|l| l.split(']').next())
            .filter(|name| *name != "tambo")
            .collect();
        let roster: Vec<&str> = ROSTER.iter().map(|(t, _)| *t).collect();
        assert_eq!(roster, listed);
    }

    #[test]
    fn roster_programs_agree_with_the_adapters() {
        for &(tool, programs) in ROSTER {
            let Some(name) = crate::model::ToolName::parse(tool) else {
                continue;
            };
            let known = crate::adapter::programs(name);
            if known.is_empty() {
                continue;
            }
            for p in programs {
                assert!(known.contains(p), "{tool}: {p} not in {known:?}");
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
            .map(|(i, &(tool, _))| Probe {
                tool,
                program: Some(PathBuf::from(format!("/bin/{tool}"))),
                outcome: match i % 4 {
                    0 => Outcome::Primed(vec!["x".into()]),
                    1 => Outcome::NoPrime,
                    2 => Outcome::TimedOut,
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
        assert!(v["primer"].get("text").is_none());
    }
}
