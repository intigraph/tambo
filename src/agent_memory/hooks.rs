use super::{
    lifecycle::{load, private_write},
    BUNDLE_NAME,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
pub fn bundle(directory: &Path) -> Value {
    let mut hooks = Vec::new();
    for harness in ["claude", "codex"] {
        for event in [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "Stop",
            "SessionEnd",
        ] {
            if harness == "codex" && event == "SessionEnd" {
                continue;
            }
            let base = format!(
                "tambo memory hook --plan {} --harness {harness} --event {event}",
                quote(&directory.display().to_string())
            );
            let evidence = format!(
                "tambo memory evidence --plan {} --harness {harness} --event {event}",
                quote(&directory.display().to_string())
            );
            let mut hook = json!({"event":event,"command":base,"timeout":10,"harnesses":[harness],
                "evidence":{"command":evidence,"max_age_seconds":3600}});
            if event == "PreToolUse" {
                hook["matcher"] = json!(if harness == "claude" {
                    "Write|Edit|MultiEdit"
                } else {
                    "apply_patch"
                });
            }
            hooks.push(hook);
        }
    }
    json!({"schema":"st.hook-bundle/1","name":BUNDLE_NAME,"version":"1.0.0","owner":"tambo","roles":["*"],"hooks":hooks})
}
fn valid(harness: &str, event: &str) -> bool {
    ["claude", "codex"].contains(&harness)
        && [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "Stop",
            "SessionEnd",
        ]
        .contains(&event)
        && !(harness == "codex" && event == "SessionEnd")
}
fn receipt_path(directory: &Path, harness: &str, event: &str) -> Result<std::path::PathBuf> {
    if !valid(harness, event) {
        bail!("unsupported memory hook event/harness");
    }
    Ok(directory
        .join("receipts")
        .join(format!("{harness}-{event}.json")))
}
fn applied_at(directory: &Path) -> Result<f64> {
    let state: Value = serde_json::from_slice(&fs::read(directory.join("state.json"))?)?;
    if state["schema"] != 1 || state["phase"] != "applied" {
        bail!("memory plan is not applied");
    }
    state["at"]
        .as_f64()
        .filter(|n| n.is_finite() && *n > 0.0)
        .context("no application time")
}
pub fn record(
    directory: &Path,
    harness: &str,
    event: &str,
    session: &str,
    exit: i32,
) -> Result<()> {
    let plan = load(directory)?;
    applied_at(directory)?;
    if session.is_empty() || session.len() > 128 || session.chars().any(char::is_control) {
        bail!("hook receipt has no usable session identity");
    }
    if exit != 0 && !(event == "PreToolUse" && exit == 2) {
        bail!("unsuccessful hook cannot produce firing evidence");
    }
    let path = receipt_path(directory, harness, event)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
    private_write(
        &path,
        &serde_json::to_vec(
            &json!({"schema":1,"plan":plan.id,"harness":harness,"event":event,
        "session":session,"completed_at":now,"exit_code":exit}),
        )?,
    )
}
pub fn evidence(directory: &Path, harness: &str, event: &str) -> Result<f64> {
    let plan = load(directory)?;
    let path = receipt_path(directory, harness, event)?;
    let meta = fs::symlink_metadata(&path)?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 4096 {
        bail!("unreadable firing receipt");
    }
    let value: Value = serde_json::from_slice(&fs::read(path)?)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
    let stamp = value["completed_at"]
        .as_f64()
        .context("receipt has no timestamp")?;
    let code = value["exit_code"]
        .as_i64()
        .context("receipt has no exit code")?;
    if value["schema"] != 1
        || value["plan"] != plan.id
        || value["harness"] != harness
        || value["event"] != event
        || !value["session"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 128)
        || !stamp.is_finite()
        || stamp < applied_at(directory)?
        || stamp > now + 5.0
        || !(code == 0 || event == "PreToolUse" && code == 2)
    {
        bail!("firing receipt does not match the applied memory plan");
    }
    Ok(stamp)
}
/// Run only supported producer hooks. The receipt is metadata, never payload
/// text. A child failure/timeout is fail-open and cannot create success evidence.
pub fn run(directory: &Path, harness: &str, event: &str) -> i32 {
    fn inner(directory: &Path, harness: &str, event: &str) -> Result<i32> {
        load(directory)?;
        applied_at(directory)?;
        receipt_path(directory, harness, event)?;
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 4 * 1024 * 1024 {
            bail!("hook input exceeds bound");
        }
        let payload: Value = serde_json::from_slice(&bytes)?;
        let session = payload["session_id"]
            .as_str()
            .or_else(|| payload["sessionId"].as_str())
            .context("no session identity")?;
        let mut calls = match event {
            "SessionStart" => vec![vec!["flush", "--hook"]],
            "UserPromptSubmit" => vec![vec!["hook", "user-prompt-submit"]],
            "PreToolUse" => vec![vec!["hook", "memory-redirect", "--mode", "block"]],
            "Stop" => vec![vec!["hook", "stop"]],
            "SessionEnd" => vec![vec!["hook", "session-end"]],
            _ => bail!("unsupported event"),
        };
        // Codex has no observed SessionEnd hook. Consolidate on its supported
        // Stop event, using the producer's existing bounded detached path.
        if harness == "codex" && event == "Stop" {
            calls.push(vec!["hook", "session-end"]);
        }
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut code = 0;
        for args in calls {
            let mut child = Command::new("cy")
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?;
            let mut input = child.stdin.take().context("child stdin unavailable")?;
            let body = bytes.clone();
            let writer = std::thread::spawn(move || input.write_all(&body));
            loop {
                if let Some(status) = child.try_wait()? {
                    code = status.code().unwrap_or(1);
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = writer.join();
                    bail!("producer hook deadline");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = writer.join();
            if code != 0 {
                break;
            }
        }
        record(directory, harness, event, session, code)?;
        Ok(if event == "PreToolUse" && code == 2 {
            2
        } else {
            0
        })
    }
    inner(directory, harness, event).unwrap_or(0)
}
