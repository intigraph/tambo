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
    assert_eq!(claude_commands(&claude, "Stop"), vec!["cv-speak"]);
    let start = claude_commands(&claude, "SessionStart");
    assert_eq!(start[0], "mine --start", "the user's own hook stays first");
    assert_eq!(start[1], command(&bin, Harness::Claude, "SessionStart"));
    assert_eq!(
        claude_commands(&claude, "UserPromptSubmit"),
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
        claude_commands(&after, "SessionStart"),
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
        claude_commands(&claude, "SessionStart"),
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
