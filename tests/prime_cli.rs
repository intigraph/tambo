//! `tambo prime`: probes stack tools on PATH, never fails on a tool without prime.
#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path, thread, time::Duration};

use assert_cmd::Command;

fn fake_tool(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

/// Run each fake once with `arg` before timing anything: macOS scans a new
/// executable on its first run, one file at a time, which under parallel tests
/// can take longer than a probe's deadline.
fn warm(dir: &Path, names_and_args: &[(&str, &str)]) {
    for (name, arg) in names_and_args {
        let _ = std::process::Command::new(dir.join(name))
            .arg(arg)
            .stdin(std::process::Stdio::null())
            .output();
    }
}

/// A PATH holding only the fakes plus the system shell utilities, so real
/// stack tools installed on the host never reach the test. Each fake answers
/// its identity probe the way the real tool does.
fn fake_stack() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path();
    fake_tool(
        bin,
        "quipu",
        r#"[ "$1" = "--help" ] && { echo "qp -- AI-native knowledge graph"; exit 0; }
[ "$1 $2" = "prime --brief" ] || exit 9
echo "quipu 0.42.0: knowledge graph for agents"
echo "Store: ok"
echo "Run \`quipu prime\` for the full primer.""#,
    );
    fake_tool(
        bin,
        "bobbin",
        r#"[ "$1" = "--version" ] && { echo "bobbin 0.29.0"; exit 0; }
echo "error: unrecognized subcommand '$1'" >&2; exit 2"#,
    );
    fake_tool(
        bin,
        "sd",
        r#"[ "$1" = "--help" ] && echo "seeds is the work-item tracker for the quipu stack"
exit 0"#,
    );
    fake_tool(
        bin,
        "yupana",
        r#"[ "$1" = "--version" ] && { echo "yupana 0.15.4"; exit 0; }
sleep 31.5 &
exec sleep 31.5"#,
    );
    // A same-named foreign program (suckless st): never run with `prime`.
    fake_tool(
        bin,
        "st",
        &format!(
            r#"[ "$1" = "--help" ] && {{ echo "usage: st [-aiv] [-c class]"; exit 1; }}
touch {}/foreign-ran"#,
            bin.display()
        ),
    );
    warm(
        bin,
        &[
            ("quipu", "--help"),
            ("bobbin", "--version"),
            ("sd", "--help"),
            ("yupana", "--version"),
            ("st", "--help"),
        ],
    );
    dir
}

fn tambo(path: &Path) -> Command {
    let mut cmd = Command::cargo_bin("tambo").unwrap();
    cmd.env("PATH", format!("{}:/bin:/usr/bin", path.display()))
        .args(["prime", "--timeout-ms", "2500"]);
    cmd
}

#[test]
fn text_lists_primed_no_prime_timed_out_foreign_and_missing_tools() {
    let stack = fake_stack();
    let out = tambo(stack.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("# Tambo agent primer"));
    assert!(text.contains("## Live state"));
    assert!(text.contains("Stack tools on PATH: 5 of 11."));
    assert!(text.contains("    quipu 0.42.0: knowledge graph for agents"));
    let bobbin = text.split("### bobbin").nth(1).unwrap();
    assert!(bobbin.contains("no prime yet"), "{text}");
    let seeds = text.split("### seeds").nth(1).unwrap();
    assert!(
        seeds.contains("no prime yet"),
        "empty output is no prime: {text}"
    );
    assert!(text.contains("### yupana") && text.contains("timed out"));
    let st = text.split("### shantytown").nth(1).unwrap();
    assert!(st.contains("not the stack tool"), "{text}");
    assert!(
        !stack.path().join("foreign-ran").exists(),
        "ran a foreign st"
    );
    assert!(text.contains("Not installed: camayoc, quechua, chaski, desire-path"));
}

#[test]
fn timeout_kills_the_whole_process_group() {
    let stack = fake_stack();
    // A sleep length no other test uses, so parallel tests cannot interfere.
    fake_tool(
        stack.path(),
        "yupana",
        r#"[ "$1" = "--version" ] && { echo "yupana 0.15.4"; exit 0; }
sleep 32.25 &
exec sleep 32.25"#,
    );
    warm(stack.path(), &[("yupana", "--version")]);
    tambo(stack.path()).arg("--brief").assert().success();
    thread::sleep(Duration::from_millis(200));
    let left = std::process::Command::new("pgrep")
        .args(["-f", "sleep 32.25"])
        .output()
        .unwrap();
    assert!(
        left.stdout.is_empty(),
        "orphaned grandchild: {}",
        String::from_utf8_lossy(&left.stdout)
    );
}

#[test]
fn live_part_stays_under_one_second_and_creates_nothing() {
    let stack = fake_stack();
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("tambo").unwrap();
    let out = cmd
        .env("PATH", format!("{}:/bin:/usr/bin", stack.path().display()))
        .env("HOME", home.path())
        .current_dir(work.path())
        .args(["prime", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["live"]["status"], "partial", "the slow stub is cut off");
    let elapsed = v["live"]["elapsed_ms"].as_u64().unwrap();
    assert!(elapsed < 1200, "live part took {elapsed} ms");
    assert_eq!(
        fs::read_dir(home.path()).unwrap().count(),
        0,
        "wrote to HOME"
    );
    assert_eq!(
        fs::read_dir(work.path()).unwrap().count(),
        0,
        "wrote to cwd"
    );
}

#[test]
fn brief_is_at_most_ten_lines() {
    let stack = fake_stack();
    let out = tambo(stack.path())
        .arg("--brief")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let lines = text.lines().count();
    assert!((5..=10).contains(&lines), "{text}");
    assert!(text.contains("Primed: quipu"), "{text}");
    assert!(text.contains("No prime yet: bobbin, seeds"), "{text}");
    assert!(text.contains("Timed out: yupana"));
    assert!(text.contains("Not the stack tool: shantytown"));
}

#[test]
fn json_follows_the_contract_shape() {
    let stack = fake_stack();
    let out = tambo(stack.path())
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["schema"], "stack-prime/1");
    assert_eq!(v["tool"], "tambo");
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert!(v["primer"]["sections"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s == "Agent workflow"));
    assert!(v["primer"]["text"]
        .as_str()
        .unwrap()
        .starts_with("# Tambo agent primer"));
    assert_eq!(v["live"]["status"], "partial");
    assert_eq!(v["status"], "degraded");
    let tools = v["live"]["summary"]["tools"].as_array().unwrap();
    let quipu = tools.iter().find(|t| t["tool"] == "quipu").unwrap();
    assert_eq!(quipu["status"], "primed");
    assert_eq!(quipu["brief"][1], "Store: ok");
    let rikuy = tools.iter().find(|t| t["tool"] == "rikuy").unwrap();
    assert_eq!(rikuy["status"], "not-installed");
}

#[test]
fn static_runs_no_tool_and_section_selects_text() {
    let stack = fake_stack();
    fake_tool(
        stack.path(),
        "quipu",
        &format!("touch {}/ran", stack.path().display()),
    );
    let out = tambo(stack.path())
        .arg("--static")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(!text.contains("## Live state"));
    assert!(!stack.path().join("ran").exists());
    let out = tambo(stack.path())
        .args(["--static", "--section", "core"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8(out)
        .unwrap()
        .starts_with("## Core commands"));
    tambo(stack.path())
        .args(["--section", "nope"])
        .assert()
        .code(2);
}
