use super::*;

fn paths(root: &Path) -> Paths {
    // Resolved, so the given and canonical config paths are the same string,
    // as they are on Linux (macOS temp dirs resolve /var to /private/var).
    let root = fs::canonicalize(root).unwrap();
    Paths {
        claude_settings: root.join("claude/settings.json"),
        codex_config: root.join("codex/config.toml"),
        state: root.join("state"),
    }
}

fn tambo(root: &Path) -> PathBuf {
    let bin = root.join("bin/tambo");
    fs::create_dir_all(bin.parent().unwrap()).unwrap();
    fs::write(&bin, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

fn j(v: &Value) -> J {
    serde_json::from_value(v.clone()).unwrap()
}

fn levels(p: &Paths) -> Vec<Level> {
    verify(p).into_iter().map(|f| f.level).collect()
}

/// What Codex 0.163 writes when the user picks "Trust all and continue" (keys
/// measured on a real session; the hash itself is Codex's own).
fn codex_trusts(p: &Paths, also_tui: bool) {
    let mut text = fs::read_to_string(&p.codex_config).unwrap();
    let doc = codex_parse(Some(text.as_bytes())).unwrap();
    let keys = codex_trust_keys(&doc, &p.codex_config);
    let name = p.codex_config.display().to_string();
    text.push_str("\n[hooks.state]\n");
    for key in keys.iter().filter(|k| k.starts_with(&format!("{name}:"))) {
        text.push_str(&format!(
            "\n[hooks.state.\"{key}\"]\ntrusted_hash = \"sha256:00\"\n"
        ));
    }
    if also_tui {
        text.push_str("\n[tui]\nscreen_reader_detection_done = true\n");
    }
    fs::write(&p.codex_config, text).unwrap();
}

const CLAUDE_BEFORE: &str = r#"{
  "model": "opus",
  "hooks": {
    "Stop": [ { "hooks": [ { "type": "command", "command": "cv-speak" } ] } ],
    "SessionStart": [ { "hooks": [ { "type": "command", "command": "mine --start" } ] } ]
  }
}
"#;

const CODEX_BEFORE: &str = "model = \"gpt\"  # keep this comment\n\n[[hooks.Stop]]\n[[hooks.Stop.hooks]]\ntype = \"command\"\ncommand = \"dp ingest\"\n";

fn seed(p: &Paths) {
    fs::create_dir_all(p.claude_settings.parent().unwrap()).unwrap();
    fs::create_dir_all(p.codex_config.parent().unwrap()).unwrap();
    fs::write(&p.claude_settings, CLAUDE_BEFORE).unwrap();
    fs::write(&p.codex_config, CODEX_BEFORE).unwrap();
}

#[test]
fn install_keeps_existing_hooks_and_adds_both_events_for_both_harnesses() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    seed(&p);
    let bin = tambo(d.path());
    let out = install(&p, &bin, &Target::default()).unwrap();
    assert_eq!(
        out,
        vec![
            (Harness::Claude, Outcome::Written),
            (Harness::Codex, Outcome::Written)
        ]
    );

    let claude: Value = serde_json::from_slice(&fs::read(&p.claude_settings).unwrap()).unwrap();
    assert_eq!(claude["model"], "opus");
    assert_eq!(claude_commands(&j(&claude), "Stop"), vec!["cv-speak"]);
    let start = claude_commands(&j(&claude), "SessionStart");
    assert_eq!(start[0], "mine --start", "the user's own hook stays first");
    assert_eq!(start[1], command(&bin, Harness::Claude, "SessionStart"));
    assert_eq!(
        claude_commands(&j(&claude), "UserPromptSubmit"),
        vec![command(&bin, Harness::Claude, "UserPromptSubmit")]
    );

    let codex = fs::read_to_string(&p.codex_config).unwrap();
    assert!(
        codex.starts_with("model = \"gpt\"  # keep this comment"),
        "{codex}"
    );
    let doc = codex_parse(Some(codex.as_bytes())).unwrap();
    assert_eq!(codex_commands(&doc, "Stop"), vec!["dp ingest"]);
    for e in EVENTS {
        assert_eq!(
            codex_commands(&doc, e),
            vec![command(&bin, Harness::Codex, e)]
        );
    }
    assert!(
        codex.contains("[[hooks.SessionStart]]") && codex.contains("[[hooks.SessionStart.hooks]]"),
        "{codex}"
    );
    assert!(levels(&p) == [Level::Ok, Level::Warn], "{:?}", verify(&p));
}

#[test]
fn a_second_install_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    seed(&p);
    let bin = tambo(d.path());
    install(&p, &bin, &Target::default()).unwrap();
    let claude = fs::read(&p.claude_settings).unwrap();
    let codex = fs::read(&p.codex_config).unwrap();
    let out = install(&p, &bin, &Target::default()).unwrap();
    assert!(out.iter().all(|(_, o)| *o == Outcome::Unchanged), "{out:?}");
    assert_eq!(fs::read(&p.claude_settings).unwrap(), claude);
    assert_eq!(fs::read(&p.codex_config).unwrap(), codex);
}

#[test]
fn uninstall_restores_the_prior_bytes_exactly() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    seed(&p);
    fs::set_permissions(&p.claude_settings, fs::Permissions::from_mode(0o640)).unwrap();
    let bin = tambo(d.path());
    install(&p, &bin, &Target::default()).unwrap();
    install(&p, &bin, &Target::default()).unwrap();
    let out = uninstall(&p).unwrap();
    assert!(out.iter().all(|(_, o)| *o == Outcome::Restored), "{out:?}");
    assert_eq!(
        fs::read_to_string(&p.claude_settings).unwrap(),
        CLAUDE_BEFORE
    );
    assert_eq!(fs::read_to_string(&p.codex_config).unwrap(), CODEX_BEFORE);
    let mode = fs::metadata(&p.claude_settings)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        mode, 0o640,
        "the file keeps its mode across install and restore"
    );
    assert!(verify(&p).iter().all(|f| f.level == Level::Warn));
}

#[test]
fn absent_files_are_created_then_removed_again() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    let bin = tambo(d.path());
    install(&p, &bin, &Target::default()).unwrap();
    assert!(p.claude_settings.exists() && p.codex_config.exists());
    uninstall(&p).unwrap();
    assert!(!p.claude_settings.exists() && !p.codex_config.exists());
}

#[test]
fn an_edit_after_install_is_kept_and_only_tambo_entries_are_removed() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    seed(&p);
    let bin = tambo(d.path());
    install(&p, &bin, &Target::default()).unwrap();
    let mut claude: Value = serde_json::from_slice(&fs::read(&p.claude_settings).unwrap()).unwrap();
    claude["theme"] = json!("dark");
    fs::write(&p.claude_settings, serde_json::to_vec(&claude).unwrap()).unwrap();
    let out = uninstall(&p).unwrap();
    assert_eq!(out[0], (Harness::Claude, Outcome::Removed));
    assert_eq!(out[1], (Harness::Codex, Outcome::Restored));
    let after: Value = serde_json::from_slice(&fs::read(&p.claude_settings).unwrap()).unwrap();
    assert_eq!(after["theme"], "dark");
    assert_eq!(
        claude_commands(&j(&after), "SessionStart"),
        vec!["mine --start"]
    );
    assert!(after["hooks"].get("UserPromptSubmit").is_none());
}

#[test]
fn a_malformed_settings_file_stops_install_before_any_write() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    seed(&p);
    fs::write(&p.codex_config, "this is = = not toml").unwrap();
    let bin = tambo(d.path());
    let err = install(&p, &bin, &Target::default()).unwrap_err();
    assert!(format!("{err:#}").contains("not valid TOML"), "{err:#}");
    assert_eq!(
        fs::read_to_string(&p.claude_settings).unwrap(),
        CLAUDE_BEFORE
    );
    assert!(!p.state.exists());
}

#[test]
fn a_moved_tambo_is_reported_broken_and_reinstall_repoints_it() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    let old = tambo(d.path());
    install(&p, &old, &Target::default()).unwrap();
    fs::remove_file(&old).unwrap();
    let findings = verify(&p);
    assert!(
        findings
            .iter()
            .all(|f| f.level == Level::Fail && f.detail.contains("tambo hooks install")),
        "{findings:?}"
    );
    let new = d.path().join("other/tambo");
    fs::create_dir_all(new.parent().unwrap()).unwrap();
    fs::write(&new, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&new, fs::Permissions::from_mode(0o755)).unwrap();
    install(&p, &new, &Target::default()).unwrap();
    assert_eq!(levels(&p), [Level::Ok, Level::Warn]);
    let claude: Value = serde_json::from_slice(&fs::read(&p.claude_settings).unwrap()).unwrap();
    assert_eq!(
        claude_commands(&j(&claude), "SessionStart"),
        vec![command(&new, Harness::Claude, "SessionStart")]
    );
}

#[test]
fn a_path_with_a_quote_round_trips_through_the_command() {
    let bin = PathBuf::from("/opt/it's here/tambo");
    let c = command(&bin, Harness::Codex, "SessionStart");
    assert_eq!(
        c,
        "'/opt/it'\"'\"'s here/tambo' hook query-first --harness codex --event SessionStart"
    );
    assert!(is_ours(&c));
}

#[test]
fn directive_names_the_store_and_says_unavailable_rather_than_empty() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("quipu.db");
    let target = Target {
        db: Some(db.clone()),
    };
    let (text, mode, reachable) = directive("SessionStart", None, &target, false);
    let text = text.unwrap();
    assert_eq!((mode, reachable), ("local", Some(false)));
    assert!(text.contains("quipu search") && text.contains(&db.display().to_string()));
    assert!(text.contains("UNAVAILABLE") && text.contains("not the same as"));
    fs::write(&db, "").unwrap();
    let (text, _, reachable) = directive("SessionStart", None, &target, false);
    assert_eq!(reachable, Some(true));
    assert!(!text.unwrap().contains("UNAVAILABLE"));

    let (text, mode, _) = directive(
        "UserPromptSubmit",
        Some("https://q.example"),
        &target,
        false,
    );
    assert_eq!(mode, "remote");
    assert!(text.unwrap().contains("https://q.example/search"));

    let (text, mode, _) = directive("SessionStart", None, &target, true);
    assert_eq!((text, mode), (None, "crew-managed"));
}

#[test]
fn codex_hooks_wait_for_trust_and_uninstall_drops_the_trust_entries() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    seed(&p);
    let bin = tambo(d.path());
    install(&p, &bin, &Target::default()).unwrap();
    let codex = verify(&p).pop().unwrap();
    assert_eq!(codex.level, Level::Warn);
    assert!(
        codex.detail.contains("not trusted") && codex.detail.contains("Trust all"),
        "{}",
        codex.detail
    );

    codex_trusts(&p, false);
    assert_eq!(levels(&p), [Level::Ok, Level::Ok]);
    let out = uninstall(&p).unwrap();
    assert_eq!(
        out[1],
        (Harness::Codex, Outcome::Restored),
        "only our trust entries were added"
    );
    assert_eq!(fs::read_to_string(&p.codex_config).unwrap(), CODEX_BEFORE);
}

#[test]
fn codex_state_written_after_trust_is_kept_on_uninstall() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    seed(&p);
    let bin = tambo(d.path());
    install(&p, &bin, &Target::default()).unwrap();
    codex_trusts(&p, true);
    let out = uninstall(&p).unwrap();
    assert_eq!(out[1], (Harness::Codex, Outcome::Removed));
    let after = fs::read_to_string(&p.codex_config).unwrap();
    assert!(after.starts_with(CODEX_BEFORE), "{after}");
    assert!(
        after.contains("screen_reader_detection_done = true"),
        "{after}"
    );
    assert!(
        !after.contains("hooks.state") && !after.contains("query-first"),
        "{after}"
    );
}

#[test]
fn install_keeps_the_users_key_order_everywhere() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    fs::create_dir_all(p.claude_settings.parent().unwrap()).unwrap();
    let before = r#"{"zeta": 1, "alpha": {"y": true, "b": null}, "hooks": {"Stop": [{"hooks": [{"command": "cv-speak", "type": "command"}]}], "PreToolUse": []}, "mid": 2.5}"#;
    fs::write(&p.claude_settings, before).unwrap();
    install(&p, &tambo(d.path()), &Target::default()).unwrap();
    let after = fs::read_to_string(&p.claude_settings).unwrap();
    let at = |needle: &str| {
        after
            .find(needle)
            .unwrap_or_else(|| panic!("{needle} missing: {after}"))
    };
    assert!(
        at("\"zeta\"") < at("\"alpha\"")
            && at("\"alpha\"") < at("\"hooks\"")
            && at("\"hooks\"") < at("\"mid\""),
        "{after}"
    );
    assert!(at("\"y\"") < at("\"b\""), "{after}");
    assert!(
        at("\"Stop\"") < at("\"PreToolUse\"") && at("\"PreToolUse\"") < at("\"SessionStart\""),
        "{after}"
    );
    assert!(
        at("\"command\": \"cv-speak\"") < at("\"type\": \"command\""),
        "the user's hook keeps its own key order: {after}"
    );
    assert!(after.contains("2.5") && after.contains("null"), "{after}");
}

/// A user hook after ours, trusted, the way Codex records it.
fn codex_with_user_hook_after_ours(p: &Paths, bin: &Path) -> String {
    install(p, bin, &Target::default()).unwrap();
    let mut text = fs::read_to_string(&p.codex_config).unwrap();
    text.push_str("\n[[hooks.SessionStart]]\n[[hooks.SessionStart.hooks]]\ntype = \"command\"\ncommand = \"mine --late\"\n");
    let name = p.codex_config.display().to_string();
    text.push_str(&format!(
        "\n[hooks.state.\"{name}:session_start:0:0\"]\ntrusted_hash = \"sha256:ours\"\n\n[hooks.state.\"{name}:session_start:1:0\"]\ntrusted_hash = \"sha256:mine\"\n"
    ));
    fs::write(&p.codex_config, &text).unwrap();
    name
}

fn trust(p: &Paths) -> Vec<(String, String)> {
    let doc = codex_parse(Some(&fs::read(&p.codex_config).unwrap())).unwrap();
    let Some(state) = doc
        .get("hooks")
        .and_then(|h| h.get("state"))
        .and_then(|s| s.as_table_like())
    else {
        return Vec::new();
    };
    state
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                v.get("trusted_hash")
                    .and_then(|h| h.as_str())
                    .unwrap()
                    .to_string(),
            )
        })
        .collect()
}

#[test]
fn repointing_replaces_our_codex_hook_in_place_so_other_trust_keys_hold() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    let name = codex_with_user_hook_after_ours(&p, &tambo(d.path()));
    let moved = d.path().join("other/tambo");
    fs::create_dir_all(moved.parent().unwrap()).unwrap();
    fs::write(&moved, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&moved, fs::Permissions::from_mode(0o755)).unwrap();
    install(&p, &moved, &Target::default()).unwrap();
    let doc = codex_parse(Some(&fs::read(&p.codex_config).unwrap())).unwrap();
    assert_eq!(
        codex_commands(&doc, "SessionStart"),
        vec![
            command(&moved, Harness::Codex, "SessionStart"),
            "mine --late".to_string()
        ],
        "ours stays at group 0, the user's stays at group 1"
    );
    assert!(trust(&p).contains(&(format!("{name}:session_start:1:0"), "sha256:mine".into())));
}

#[test]
fn uninstall_moves_the_trust_of_hooks_that_shift_up() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    let name = codex_with_user_hook_after_ours(&p, &tambo(d.path()));
    uninstall(&p).unwrap();
    let doc = codex_parse(Some(&fs::read(&p.codex_config).unwrap())).unwrap();
    assert_eq!(codex_commands(&doc, "SessionStart"), vec!["mine --late"]);
    assert_eq!(
        trust(&p),
        vec![(
            format!("{name}:session_start:0:0"),
            "sha256:mine".to_string()
        )],
        "the user's trust moves with its hook from group 1 to group 0; ours is gone"
    );
}

#[test]
fn a_symlinked_settings_file_is_refused_and_left_a_symlink() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    seed(&p);
    let real = d.path().join("dotfiles/settings.json");
    fs::create_dir_all(real.parent().unwrap()).unwrap();
    fs::rename(&p.claude_settings, &real).unwrap();
    std::os::unix::fs::symlink(&real, &p.claude_settings).unwrap();
    let err = install(&p, &tambo(d.path()), &Target::default()).unwrap_err();
    assert!(format!("{err:#}").contains("is a symlink"), "{err:#}");
    assert!(fs::symlink_metadata(&p.claude_settings)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(&real).unwrap(), CLAUDE_BEFORE);
    assert_eq!(
        fs::read_to_string(&p.codex_config).unwrap(),
        CODEX_BEFORE,
        "no harness is written"
    );
    assert!(uninstall(&p).is_err());
}

#[test]
fn crew_hosts_are_recognised() {
    use std::ffi::OsStr;
    assert_eq!(
        crew_host_reason(Some(OsStr::new("/r")), false).as_deref(),
        Some("SHANTY_ROOT is set")
    );
    assert_eq!(
        crew_host_reason(None, true).as_deref(),
        Some("the plan selects a shantytown crew")
    );
    assert_eq!(crew_host_reason(Some(OsStr::new("")), false), None);
    assert_eq!(crew_host_reason(None, false), None);
}
