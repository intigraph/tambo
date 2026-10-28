//! `tambo prime`: probes stack tools on PATH, never fails on a tool without prime.
#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path, time::Instant};

use assert_cmd::Command;

fn fake_tool(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

/// A PATH holding only the fakes plus the system shell utilities, so real
/// stack tools installed on the host never reach the test.
fn fake_stack() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path();
    fake_tool(
        bin,
        "quipu",
        r#"[ "$1 $2" = "prime --brief" ] || exit 9
echo "quipu 0.42.0: knowledge graph for agents"
echo "Store: ok"
echo "Run \`quipu prime\` for the full primer.""#,
    );
    fake_tool(
        bin,
        "bobbin",
        "echo \"error: unrecognized subcommand '$1'\" >&2; exit 2",
    );
    fake_tool(bin, "sd", "exit 0");
    fake_tool(bin, "yupana", "exec sleep 30");
    dir
}

fn tambo(path: &Path) -> Command {
    let mut cmd = Command::cargo_bin("tambo").unwrap();
    cmd.env("PATH", format!("{}:/bin:/usr/bin", path.display()))
        .args(["prime", "--timeout-ms", "2500"]);
    cmd
}

#[test]
fn text_lists_primed_no_prime_timed_out_and_missing_tools() {
    let stack = fake_stack();
    let started = Instant::now();
    let out = tambo(stack.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(
        started.elapsed().as_secs() < 8,
        "probes did not run in parallel"
    );
    assert!(text.starts_with("# Tambo agent primer"));
    assert!(text.contains("## Live state"));
    assert!(text.contains("Stack tools on PATH: 4 of 11."));
    assert!(text.contains("    quipu 0.42.0: knowledge graph for agents"));
    let bobbin = text.split("### bobbin").nth(1).unwrap();
    assert!(bobbin.contains("no prime yet"), "{text}");
    let seeds = text.split("### seeds").nth(1).unwrap();
    assert!(
        seeds.contains("no prime yet"),
        "empty output is no prime: {text}"
    );
    assert!(text.contains("### yupana") && text.contains("timed out"));
    assert!(text.contains("Not installed: camayoc, quechua, chaski, desire-path"));
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
    assert!(text.contains("Primed: quipu"));
    assert!(text.contains("No prime yet: bobbin, seeds"));
    assert!(text.contains("Timed out: yupana"));
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
