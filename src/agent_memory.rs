//! Opt-in memory setup through the existing settings owner. Configuration is
//! not firing evidence, and firing is not promotion/migration acceptance.
use std::{collections::BTreeSet, path::Path};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use toml_edit::{value, DocumentMut};

pub const BUNDLE_NAME: &str = "camayoc-memory";

pub mod hooks;
pub mod lifecycle;
#[cfg(test)]
mod tests;

/// Build the exact reviewable edits without changing a host. Bobbin continues
/// to own recall; no second recall hook or MCP server is invented here.
pub fn render(root_config: &str, bobbin_config: &str, person: &str) -> Result<(String, String)> {
    if person.is_empty() || person.chars().any(|c| c.is_whitespace() || c.is_control()) {
        bail!("person must be a nonempty IRI or local entity name");
    }
    let mut root: DocumentMut = root_config.parse().context("parse shantytown config")?;
    let mut bobbin: DocumentMut = bobbin_config.parse().context("parse bobbin config")?;
    for (key, val) in [
        ("CAMAYOC_CAPTURE", "1"),
        ("CAMAYOC_MEMORY_REDIRECT", "block"),
        ("CAMAYOC_PERSON", person),
        ("SHANTY_STOP_RULING_MEMORY", "1"),
        ("BOBBIN_MEMORY_RECALL", "1"),
    ] {
        if root.get("env").is_some_and(|v| !v.is_table_like()) {
            bail!("shantytown [env] is not a table");
        }
        root["env"][key] = value(val);
    }
    if bobbin.get("context").is_some_and(|v| !v.is_table_like()) {
        bail!("bobbin [context] is not a table");
    }
    // Preserve a deliberate nondefault budget. Zero cannot meet this plan.
    match bobbin
        .get("context")
        .and_then(|v| v.get("knowledge_budget_pct"))
    {
        Some(v) => {
            let pct = v.as_float().or_else(|| v.as_integer().map(|n| n as f64));
            if !pct.is_some_and(|n| n.is_finite() && n > 0.0 && n <= 100.0) {
                bail!("memory recall requires knowledge_budget_pct in (0,100]");
            }
        }
        None => bobbin["context"]["knowledge_budget_pct"] = value(15.0),
    }
    Ok((root.to_string(), bobbin.to_string()))
}

/// Strict view of the settings owner's real report. Every required item must
/// be configured, carried by a live harness, AND observed firing. We never
/// translate a not-checked/unknown report to success or fabricate timestamps.
pub fn assess(report: &Value) -> Result<Value> {
    if report["schema"] != "st.hook-check/1" {
        bail!("unsupported settings-owner hook report");
    }
    if !report
        .get("registry_errors")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        bail!("settings owner cannot establish an error-free hook registry");
    }
    let items = report["items"]
        .as_array()
        .context("owner report has no items")?;
    let mut pending = BTreeSet::new();
    for harness in ["claude", "codex"] {
        for (name, event) in [
            ("bobbin", "UserPromptSubmit"),
            (BUNDLE_NAME, "SessionStart"),
            (BUNDLE_NAME, "UserPromptSubmit"),
            (BUNDLE_NAME, "PreToolUse"),
            (BUNDLE_NAME, "Stop"),
        ] {
            let rows: Vec<_> = items
                .iter()
                .filter(|r| r["harness"] == harness && r["bundle"] == name && r["event"] == event)
                .collect();
            if rows.is_empty()
                || rows.iter().any(|r| r["configured"] != "ok")
                || !rows
                    .iter()
                    .any(|r| r["live"] == "ok" && r["firing"] == "ok")
            {
                pending.insert(format!("{harness}: {name} {event} firing not proven"));
            }
        }
        if harness == "claude"
            && !items.iter().any(|r| {
                r["harness"] == harness
                    && r["bundle"] == BUNDLE_NAME
                    && r["event"] == "SessionEnd"
                    && r["configured"] == "ok"
                    && r["live"] == "ok"
                    && r["firing"] == "ok"
            })
        {
            pending.insert("claude: camayoc-memory SessionEnd firing not proven".to_string());
        }
    }
    let hook_setup_status = if pending.is_empty() {
        "verified"
    } else {
        "pending"
    };
    let hook_pending = pending;
    let mut pending = BTreeSet::new();
    // These proofs have separate producers. A recent hook timestamp alone
    // cannot establish any of them; never flip auto-memory from this report.
    for gate in [
        "migration and conflict clearance",
        "18 of 20 recall prompts without superseded hits",
        "native Stop ruling reminder",
        "typed promotion within ten minutes for both harnesses",
        "SessionStart Handoff and Directive pins",
    ] {
        pending.insert(format!("acceptance pending: {gate}"));
    }
    Ok(
        json!({"schema":"tambo.agent-memory-verification/1", "status":"pending",
        "hook_setup_status":hook_setup_status,"hook_pending":hook_pending,
        "unproven_carriers":items.iter().filter(|r| (r["bundle"]==BUNDLE_NAME || r["bundle"]=="bobbin") && (r["live"]!="ok" || r["firing"]!="ok")).map(|r|json!({"harness":r["harness"],"role":r["role"],"event":r["event"],"live":r["live"],"firing":r["firing"]})).collect::<Vec<_>>(),
        "auto_memory_changed":false, "auto_memory_eligible":false,"auto_memory_blockers":pending}),
    )
}

/// Verify the exact plan's current files and the owner's live firing report.
/// No functional writes, hook invocation, or auto-memory mutation occurs here.
pub fn verify(directory: &Path, runner: &mut impl lifecycle::Runner) -> Result<Value> {
    let plan = lifecycle::load(directory)?;
    let owner = runner.report(
        "st",
        &[
            "--root".into(),
            plan.root.display().to_string(),
            "ops".into(),
            "hooks".into(),
            "check".into(),
            "--json".into(),
        ],
    )?;
    let owner: Value = serde_json::from_str(&owner).context("no readable owner hook report")?;
    let mut result = assess(&owner)?;
    let mut drift = Vec::new();
    if owner["root"] != plan.root.display().to_string() {
        drift.push("owner report is for another or unknown store".to_string());
    }
    for bundle in &plan.bundles {
        if owner["items"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|r| r["bundle"] == bundle["name"] && r["version"] != bundle["version"])
        {
            drift.push("registered bundle version differs from plan".to_string());
        }
    }
    let state = std::fs::read(directory.join("state.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    if state.as_ref().map(|s| &s["phase"]) != Some(&json!("applied")) {
        drift.push("memory plan is not applied".to_string());
    }
    for change in &plan.changes {
        if std::fs::read_to_string(&change.path).ok().as_deref() != Some(change.after.as_str()) {
            drift.push(format!(
                "configured file differs: {}",
                change.path.display()
            ));
        }
    }
    for args in [
        vec!["--version"],
        vec!["record", "decision", "--help"],
        vec!["recall", "decision", "--help"],
    ] {
        if runner
            .run(
                "cy",
                &args.iter().map(|s| (*s).to_string()).collect::<Vec<_>>(),
            )
            .is_err()
        {
            drift.push("generated Camayoc CLI tools unavailable".to_string());
        }
    }
    if lifecycle::check_tools(&plan, runner).is_err() {
        drift.push("Camayoc generated tool schemas differ or cannot be read".to_string());
    }
    if !drift.is_empty() {
        result["hook_setup_status"] = json!("pending");
    }
    result["configuration_drift"] = json!(drift);
    result["plan_id"] = json!(plan.id);
    result["source_revision"] = json!(plan.source_revision);
    Ok(result)
}

/// The rollout decision is separate from installing the setup. A future
/// acceptance producer must supply migration/recall/promotion/pin proof; no
/// caller-supplied bool or a recent hook timestamp can bypass that gate.
pub fn disable_auto_memory(directory: &Path, runner: &mut impl lifecycle::Runner) -> Result<()> {
    let report = verify(directory, runner)?;
    bail!(
        "auto-memory remains unchanged: {}",
        report["auto_memory_blockers"]
    );
}
