use super::{
    assess, hooks,
    lifecycle::{self, Runner},
    render, verify, BUNDLE_NAME,
};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

struct Owner {
    root: PathBuf,
    registry: Vec<Value>,
    calls: Vec<Vec<String>>,
    fail_cy: bool,
    fail_register: bool,
    report: Value,
    wrong_tools: bool,
}
impl Owner {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            registry: vec![],
            calls: vec![],
            fail_cy: false,
            fail_register: false,
            report: full_report(),
            wrong_tools: false,
        }
    }
}
impl Runner for Owner {
    fn run(&mut self, program: &str, args: &[String]) -> Result<String> {
        self.calls.push(
            std::iter::once(program.to_string())
                .chain(args.iter().cloned())
                .collect(),
        );
        if program == "git" {
            if args.last().is_some_and(|s| s.ends_with("mcp-tools.json")) {
                return Ok(json!({"schema":"camayoc-mcp-tools/1","server":"camayoc-memory","tools":[{"name":"record_decision","inputSchema":{"type":"object"}}]}).to_string());
            }
            return Ok(if args.iter().any(|s| s == "rev-parse") {
                "a".repeat(40)
            } else {
                "---\nname: camayoc-memory\n---\nGenerated producer skill: cy record decision; cy recall decision\n".to_string()
            });
        }
        if program == "bobbin" {
            return Ok(crate::hook_bundles::BUNDLES[0].1.to_string());
        }
        if program == "cy" {
            if self.fail_cy {
                bail!("fixture missing CLI");
            }
            return Ok("camayoc 0.1.4 --hook".to_string());
        }
        if program != "st" {
            bail!("unexpected command");
        }
        if args.iter().any(|s| s == "list") {
            return Ok(json!({"bundles":self.registry,"errors":[]}).to_string());
        }
        if args.iter().any(|s| s == "check") {
            let mut report = self.report.clone();
            report["root"] = json!(args[1]);
            return Ok(report.to_string());
        }
        if args.iter().any(|s| s == "provision") {
            return Ok(json!({"version":1,"owner":"shantytown","agents":[{"name":"fixture","servers":["existing-owner-server"]}]}).to_string());
        }
        let directory = self.root.join("hook-bundles");
        fs::create_dir_all(&directory)?;
        if args.iter().any(|s| s == "register") {
            if self.fail_register {
                bail!("fixture registration refused");
            }
            let value: Value = serde_json::from_slice(&fs::read(args.last().unwrap())?)?;
            let name = value["name"].as_str().unwrap();
            self.registry.retain(|r| r["name"] != name);
            fs::write(
                directory.join(format!("{name}.json")),
                serde_json::to_vec(&value)?,
            )?;
            self.registry.push(value.clone());
            return Ok(format!("{name}: installed"));
        }
        if args.iter().any(|s| s == "unregister") {
            let name = args.last().unwrap();
            self.registry.retain(|r| r["name"] != *name);
            fs::remove_file(directory.join(format!("{name}.json")))?;
            return Ok("removed".to_string());
        }
        bail!("unexpected owner command")
    }
    fn input(&mut self, program: &str, args: &[String], body: &str) -> Result<String> {
        assert_eq!(program, "cy");
        assert_eq!(args, &["mcp".to_string()]);
        assert!(!body.contains("tools/call"));
        assert!(body.contains("tools/list"));
        if self.wrong_tools {
            return Ok(format!(
                "{}\n{}\n",
                json!({"id":1,"result":{"serverInfo":{"name":"camayoc-memory"}}}),
                json!({"id":2,"result":{"tools":[]}})
            ));
        }
        Ok(format!(
            "{}\n{}\n",
            json!({"id":1,"result":{"serverInfo":{"name":"camayoc-memory"}}}),
            json!({"id":2,"result":{"tools":[{"name":"record_decision","inputSchema":{"type":"object"}}]}})
        ))
    }
}
fn prepare(root: &Path) -> (PathBuf, Owner) {
    prepare_mode(root, super::RedirectMode::Copy)
}
fn prepare_mode(root: &Path, mode: super::RedirectMode) -> (PathBuf, Owner) {
    let crew = root.join("crew");
    fs::create_dir_all(&crew).unwrap();
    fs::write(
        crew.join("shantytown.toml"),
        "# original\n[env]\nUNRELATED = 'keep'\n",
    )
    .unwrap();
    let bobbin = root.join("bobbin.toml");
    fs::write(
        &bobbin,
        "# budget\n[context]\nknowledge_budget_pct = 20.0\n",
    )
    .unwrap();
    let output = root.join("memory-plan");
    let mut owner = Owner::new(crew.clone());
    lifecycle::create(
        lifecycle::Create {
            root: &crew,
            bobbin: &bobbin,
            person: "person-fixture",
            output: &output,
            source: &root.join("source"),
            claude_home: &root.join("claude"),
            codex_home: &root.join("codex"),
            issuer_name: Some("Fixture"),
            redirect_mode: mode,
        },
        &mut owner,
    )
    .unwrap();
    (output, owner)
}
fn full_report() -> Value {
    let mut rows = Vec::new();
    for harness in ["claude", "codex"] {
        rows.push(json!({"harness":harness,"bundle":"bobbin","version":"0.21.0","event":"UserPromptSubmit","configured":"ok","live":"ok","firing":"ok"}));
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
            rows.push(json!({"harness":harness,"bundle":BUNDLE_NAME,"version":"1.0.0","event":event,"configured":"ok","live":"ok","firing":"ok"}));
        }
    }
    json!({"schema":"st.hook-check/1","items":rows,"registry_errors":[]})
}

#[test]
fn rendering_preserves_choices_and_never_touches_auto_memory() {
    let (crew, bobbin) = render(
        "# keep\n[env]\nEXTRA='x'\n",
        "[context]\nknowledge_budget_pct=20\n",
        "person-fixture",
    )
    .unwrap();
    assert!(crew.contains("# keep") && crew.contains("EXTRA"));
    assert!(crew.contains("SHANTY_STOP_RULING_MEMORY") && crew.contains("CAMAYOC_MEMORY_REDIRECT"));
    assert!(bobbin.contains("knowledge_budget_pct=20"));
    assert!(!crew.contains("autoMemoryEnabled"));
    let (_, default) = render("", "", "person-fixture").unwrap();
    assert!(default.contains("15.0"));
    for input in [
        "[context]\nknowledge_budget_pct=0\n",
        "[context]\nknowledge_budget_pct=101\n",
        "context='bad'\n",
    ] {
        assert!(render("", input, "person-fixture").is_err());
    }
    assert!(render("env='bad'", "", "person-fixture").is_err());
}
#[test]
fn private_plan_does_not_mutate_targets_and_pins_generated_source() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let (plan, _) = prepare(temp.path());
    let loaded = lifecycle::load(&plan).unwrap();
    assert_eq!(loaded.source_revision, "a".repeat(40));
    assert_eq!(
        fs::metadata(&plan).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(plan.join("plan.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(fs::read_to_string(loaded.root.join("shantytown.toml"))
        .unwrap()
        .contains("UNRELATED = 'keep'"));
    assert!(!temp
        .path()
        .join("claude/skills/camayoc-memory/SKILL.md")
        .exists());
}
#[test]
fn apply_and_restore_are_lossless_and_keep_prior_bundle_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    let plan = lifecycle::load(&directory).unwrap();
    let before = fs::read(plan.root.join("shantytown.toml")).unwrap();
    let old = json!({"schema":"st.hook-bundle/1","name":"bobbin","version":"prior","owner":"producer","roles":["*"],
        "hooks":[{"event":"UserPromptSubmit","command":"bobbin hook old","evidence":{"command":"producer-evidence","max_age_seconds":42}}]});
    fs::create_dir_all(plan.root.join("hook-bundles")).unwrap();
    fs::write(
        plan.root.join("hook-bundles/bobbin.json"),
        serde_json::to_vec(&old).unwrap(),
    )
    .unwrap();
    owner.registry.push(old.clone());
    lifecycle::apply(&directory, &mut owner).unwrap();
    assert!(temp
        .path()
        .join("claude/skills/camayoc-memory/SKILL.md")
        .exists());
    assert!(temp
        .path()
        .join("codex/skills/camayoc-memory/SKILL.md")
        .exists());
    lifecycle::apply(&directory, &mut owner).unwrap();
    lifecycle::restore(&directory, &mut owner).unwrap();
    assert_eq!(fs::read(plan.root.join("shantytown.toml")).unwrap(), before);
    assert!(!temp
        .path()
        .join("codex/skills/camayoc-memory/SKILL.md")
        .exists());
    assert!(!temp.path().join("claude/skills/camayoc-memory").exists());
    assert!(!temp.path().join("codex/skills/camayoc-memory").exists());
    let restored: Value =
        serde_json::from_slice(&fs::read(plan.root.join("hook-bundles/bobbin.json")).unwrap())
            .unwrap();
    assert_eq!(restored, old);
    let stamp = fs::metadata(plan.root.join("shantytown.toml"))
        .unwrap()
        .modified()
        .unwrap();
    lifecycle::restore(&directory, &mut owner).unwrap();
    assert_eq!(
        fs::metadata(plan.root.join("shantytown.toml"))
            .unwrap()
            .modified()
            .unwrap(),
        stamp
    );
}
#[test]
fn edits_since_review_and_since_apply_are_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    let path = owner.root.join("shantytown.toml");
    let before = fs::read(&path).unwrap();
    fs::write(&path, "# operator edit\n").unwrap();
    assert!(lifecycle::apply(&directory, &mut owner).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "# operator edit\n");
    fs::write(&path, before).unwrap();
    lifecycle::apply(&directory, &mut owner).unwrap();
    fs::write(&path, "# later edit\n").unwrap();
    assert!(lifecycle::restore(&directory, &mut owner).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "# later edit\n");
}
#[test]
fn missing_prerequisite_and_changed_staged_bundle_cannot_write_targets() {
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    let path = owner.root.join("shantytown.toml");
    let before = fs::read(&path).unwrap();
    owner.fail_cy = true;
    assert!(lifecycle::apply(&directory, &mut owner).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    owner.fail_cy = false;
    fs::write(directory.join("bundle-1.json"), "{}").unwrap();
    assert!(lifecycle::apply(&directory, &mut owner).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
}
#[test]
fn partial_application_remains_restorable() {
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    let path = owner.root.join("shantytown.toml");
    let before = fs::read(&path).unwrap();
    owner.fail_register = true;
    assert!(lifecycle::apply(&directory, &mut owner).is_err());
    let state: Value =
        serde_json::from_slice(&fs::read(directory.join("state.json")).unwrap()).unwrap();
    assert_eq!(state["phase"], "applying");
    owner.fail_register = false;
    lifecycle::restore(&directory, &mut owner).unwrap();
    assert_eq!(fs::read(path).unwrap(), before);
}
#[test]
fn configured_and_live_are_not_firing_and_one_harness_is_insufficient() {
    for value in ["unknown", "not-checked", "silent"] {
        let mut report = full_report();
        report["items"][1]["firing"] = json!(value);
        assert_eq!(assess(&report).unwrap()["hook_setup_status"], "pending");
    }
    let mut report = full_report();
    report["items"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["harness"] != "codex");
    assert_eq!(assess(&report).unwrap()["hook_setup_status"], "pending");
    assert!(assess(&json!({"schema":"unsupported","items":[]})).is_err());
}
#[test]
fn hook_success_never_approves_disabling_auto_memory() {
    let report = assess(&full_report()).unwrap();
    assert_eq!(report["hook_setup_status"], "verified");
    assert_eq!(report["auto_memory_eligible"], false);
    assert_eq!(report["auto_memory_changed"], false);
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    lifecycle::apply(&directory, &mut owner).unwrap();
    let user = temp.path().join("claude/settings.json");
    fs::write(&user, "{\"autoMemoryEnabled\":true,\"other\":1}").unwrap();
    let bytes = fs::read(&user).unwrap();
    let time = fs::metadata(&user).unwrap().modified().unwrap();
    assert!(super::disable_auto_memory(&directory, &mut owner).is_err());
    assert_eq!(fs::read(&user).unwrap(), bytes);
    assert_eq!(fs::metadata(&user).unwrap().modified().unwrap(), time);
}
#[test]
fn verification_checks_actual_files_as_well_as_owner_report() {
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    assert_eq!(
        verify(&directory, &mut owner).unwrap()["hook_setup_status"],
        "pending"
    );
    lifecycle::apply(&directory, &mut owner).unwrap();
    assert_eq!(
        verify(&directory, &mut owner).unwrap()["hook_setup_status"],
        "verified"
    );
    fs::remove_file(temp.path().join("codex/skills/camayoc-memory/SKILL.md")).unwrap();
    assert_eq!(
        verify(&directory, &mut owner).unwrap()["hook_setup_status"],
        "pending"
    );
}
#[test]
fn receipts_require_current_applied_plan_success_and_session_identity() {
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    assert!(hooks::record(&directory, "codex", "Stop", "session", 0).is_err());
    lifecycle::apply(&directory, &mut owner).unwrap();
    assert!(hooks::record(&directory, "codex", "Stop", "", 0).is_err());
    assert!(hooks::record(&directory, "codex", "Stop", "session", 1).is_err());
    assert!(hooks::record(&directory, "codex", "SessionEnd", "session", 0).is_err());
    hooks::record(&directory, "codex", "Stop", "session", 0).unwrap();
    assert!(hooks::evidence(&directory, "codex", "Stop").unwrap() > 0.0);
    assert!(hooks::evidence(&directory, "claude", "Stop").is_err());
    lifecycle::restore(&directory, &mut owner).unwrap();
    assert!(hooks::evidence(&directory, "codex", "Stop").is_err());
}
#[test]
fn receipts_do_not_store_prompt_text_and_malformed_receipts_never_prove_firing() {
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    lifecycle::apply(&directory, &mut owner).unwrap();
    hooks::record(&directory, "claude", "PreToolUse", "fixture-session", 2).unwrap();
    let path = directory.join("receipts/claude-PreToolUse.json");
    let v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert!(v.get("text").is_none() && v.get("prompt").is_none());
    fs::write(path, "{partial").unwrap();
    assert!(hooks::evidence(&directory, "claude", "PreToolUse").is_err());
}
#[test]
fn plan_tampering_or_nonprivate_plan_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let (directory, _) = prepare(temp.path());
    let path = directory.join("plan.json");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(lifecycle::load(&directory).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let mut v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    v["source_revision"] = json!("b".repeat(40));
    fs::write(path, serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(lifecycle::load(&directory).is_err());
}

#[test]
fn generated_schema_mismatch_refuses_before_configuration_writes() {
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    let path = owner.root.join("shantytown.toml");
    let before = fs::read(&path).unwrap();
    owner.wrong_tools = true;
    assert!(lifecycle::apply(&directory, &mut owner).is_err());
    assert_eq!(fs::read(path).unwrap(), before);
}
#[test]
fn edited_registered_evidence_is_preserved_on_restore() {
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    lifecycle::apply(&directory, &mut owner).unwrap();
    let path = owner.root.join("hook-bundles/camayoc-memory.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["hooks"][0]["evidence"]["command"] = json!("operator-custom-evidence");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(lifecycle::restore(&directory, &mut owner).is_err());
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(),
        value
    );
}
#[test]
fn idle_roles_are_reported_without_demanding_a_fleet_restart() {
    let mut report = full_report();
    let mut idle = report["items"][1].clone();
    idle["live"] = json!("not-checked");
    idle["firing"] = json!("not-checked");
    report["items"].as_array_mut().unwrap().push(idle);
    assert_eq!(assess(&report).unwrap()["hook_setup_status"], "verified");
}
#[test]
fn bundle_does_not_emit_unsupported_codex_events_or_a_second_recall_hook() {
    let bundle = hooks::bundle(Path::new("/tmp/private plan"), super::RedirectMode::Copy);
    for row in bundle["hooks"].as_array().unwrap() {
        assert!(!row["command"].as_str().unwrap().contains("inject-context"));
        if row["harnesses"][0] == "codex" {
            assert_ne!(row["event"], "SessionEnd");
            assert_ne!(row["event"], "PreCompact");
        }
        assert!(row["evidence"]["command"]
            .as_str()
            .unwrap()
            .contains("--plan '/tmp/private plan'"));
    }
}

#[test]
fn redirect_choice_is_pinned_and_off_needs_no_pre_tool_firing() {
    use super::RedirectMode;
    for mode in [RedirectMode::Copy, RedirectMode::Block, RedirectMode::Off] {
        let temp = tempfile::tempdir().unwrap();
        let (directory, mut owner) = prepare_mode(temp.path(), mode);
        let plan = lifecycle::load(&directory).unwrap();
        assert_eq!(plan.redirect_mode, mode);
        let config: toml::Value = plan
            .changes
            .iter()
            .find(|c| c.path.ends_with("shantytown.toml"))
            .unwrap()
            .after
            .parse()
            .unwrap();
        assert_eq!(
            config["env"]["CAMAYOC_MEMORY_REDIRECT"].as_str(),
            Some(mode.as_str())
        );
        let pre: Vec<_> = plan.bundles[1]["hooks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|h| h["event"] == "PreToolUse")
            .collect();
        if mode == RedirectMode::Off {
            assert!(pre.is_empty());
            assert!(hooks::producer_calls("claude", "PreToolUse", mode)
                .unwrap()
                .is_empty());
        } else {
            assert_eq!(pre.len(), 2);
            assert_eq!(
                hooks::producer_calls("claude", "PreToolUse", mode).unwrap(),
                vec![vec!["hook", "memory-redirect", "--mode", mode.as_str()]]
            );
        }
        lifecycle::apply(&directory, &mut owner).unwrap();
        owner.report["items"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["event"] != "PreToolUse");
        let result = verify(&directory, &mut owner).unwrap();
        assert_eq!(
            result["hook_setup_status"],
            if mode == RedirectMode::Off {
                "verified"
            } else {
                "pending"
            }
        );
        assert_eq!(result["auto_memory_eligible"], false);
    }
    assert_eq!(RedirectMode::default(), RedirectMode::Copy);
}

#[test]
fn standalone_redirect_is_refused_in_markers_and_parsed_commands() {
    for (relative, text) in [
        ("claude/settings.json", r#"{"hooks":{"PreToolUse":[{"hooks":[{"command":"cy hook memory\u002dredirect --mode copy"}]}]}}"#),
        ("codex/config.toml", "# >>> tambo: camayoc memory-redirect hook (aegis-gshu.1) >>>\n"),
        ("codex/config.toml", "[[hooks.PreToolUse]]\n[[hooks.PreToolUse.hooks]]\ncommand='cy hook memory-redirect --mode block'\n"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        let before = fs::read(&path).unwrap();
        let mut owner = Owner::new(temp.path().join("crew"));
        let result = lifecycle::create(lifecycle::Create {
            root: &temp.path().join("crew"), bobbin: &temp.path().join("bobbin.toml"), person: "person-fixture",
            output: &temp.path().join("plan"), source: &temp.path().join("source"),
            claude_home: &temp.path().join("claude"), codex_home: &temp.path().join("codex"),
            issuer_name: None, redirect_mode: super::RedirectMode::Copy,
        }, &mut owner);
        assert!(result.unwrap_err().to_string().contains("uninstall"));
        assert!(!temp.path().join("plan").exists());
        assert_eq!(before, fs::read(path).unwrap());
    }
}

#[test]
fn standalone_redirect_added_after_planning_refuses_apply_before_targets() {
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    let plan = lifecycle::load(&directory).unwrap();
    let path = temp.path().join("claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"command":"cy hook memory-redirect"}]}]}}"#,
    )
    .unwrap();
    assert!(lifecycle::apply(&directory, &mut owner)
        .unwrap_err()
        .to_string()
        .contains("uninstall"));
    for change in &plan.changes {
        let now = fs::read_to_string(&change.path).ok();
        assert_eq!(now, change.before);
    }
}

#[test]
fn restore_preserves_preexisting_or_nonempty_skill_directories() {
    let temp = tempfile::tempdir().unwrap();
    let existing = temp.path().join("claude/skills/camayoc-memory");
    fs::create_dir_all(&existing).unwrap();
    let (directory, mut owner) = prepare(temp.path());
    lifecycle::apply(&directory, &mut owner).unwrap();
    let added = temp.path().join("codex/skills/camayoc-memory/keep.txt");
    fs::write(&added, "user addition").unwrap();
    lifecycle::restore(&directory, &mut owner).unwrap();
    assert!(existing.is_dir());
    assert_eq!(fs::read_to_string(added).unwrap(), "user addition");
    lifecycle::restore(&directory, &mut owner).unwrap();
    assert!(existing.is_dir());
}

#[test]
fn reapply_takes_fresh_skill_directory_ownership_after_restore() {
    let temp = tempfile::tempdir().unwrap();
    let (directory, mut owner) = prepare(temp.path());
    lifecycle::apply(&directory, &mut owner).unwrap();
    lifecycle::restore(&directory, &mut owner).unwrap();
    let user_dir = temp.path().join("claude/skills/camayoc-memory");
    fs::create_dir_all(&user_dir).unwrap();
    lifecycle::apply(&directory, &mut owner).unwrap();
    lifecycle::restore(&directory, &mut owner).unwrap();
    assert!(user_dir.is_dir());
    assert!(!temp.path().join("codex/skills/camayoc-memory").exists());
}
