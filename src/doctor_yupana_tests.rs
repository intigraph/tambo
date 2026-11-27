use super::*;

/// `yupana status --json` (yupana 0.19.3), with the knobs each fixture turns.
fn status(mode: &str, source: &str, auth: &str) -> String {
    format!(
        r#"{{
  "base_ref": "main",
  "policy": {{"enforcing_without_scope": false, "mode": "{mode}", "scope": "fleet", "scope_configured": true}},
  "policy_mode_provenance": {{"effective": "{mode}", "lowered_by_project": false, "source": "{source}", "user_mode": null}},
  "quipu": {{"auth": {{"session_scope": "harness_session", "state": "{auth}"}}, "enabled": true}},
  "rule_set": {{"digest": "sha256:6a73e0b6", "projected": 12, "state": "loaded"}},
  "signed_rule_set": {{"loaded": true, "state": "loaded"}},
  "tenant": "worker",
  "tiers": ["treesitter"]
}}"#
    )
}

const USER_CONFIG: &str = "/home/op/.config/bobbin/config.toml";

const HOOKS_BOTH: &str =
    "claude: 198 hook item(s) via st; configured ok 198, live ok 30, firing ok 20, silent 0\n\
codex: 8 hook item(s) via st; configured ok 8, live ok 8, firing ok 4, silent 0\n";

const HOOKS_CODEX_ZERO: &str =
    "claude: 198 hook item(s) via st; configured ok 198, live ok 30, firing ok 20, silent 0\n\
codex: 0 hook item(s) via st; configured ok 0, live ok 0, firing ok 0, silent 0\n";

fn everywhere(_: &str) -> bool {
    true
}

fn warnings(findings: &[Finding]) -> Vec<&Finding> {
    findings.iter().filter(|f| f.level != Level::Ok).collect()
}

#[test]
fn guarding_host_is_clean() {
    let got = guard_findings(
        None,
        &status("enforce", USER_CONFIG, "accepted"),
        Some(HOOKS_BOTH),
        &everywhere,
    );
    assert!(warnings(&got).is_empty(), "{got:#?}");
    let head = &got[0];
    assert_eq!(head.subject, "yupana guard [worker]");
    for part in [
        "policy mode enforce",
        USER_CONFIG,
        "scope fleet",
        "12 projected rule(s)",
        "sha256:6a73e0b6",
        "quipu auth accepted",
        "signed resident cache present",
    ] {
        assert!(
            head.detail.contains(part),
            "{part} missing from {}",
            head.detail
        );
    }
    assert!(got.iter().any(|f| f.subject.ends_with("hooks codex")
        && f.detail.contains("configured 8, live 8, firing 4")));
}

#[test]
fn mode_off_from_compiled_default_warns() {
    let got = guard_findings(
        None,
        &status("off", "compiled default", "accepted"),
        Some(HOOKS_BOTH),
        &everywhere,
    );
    assert_eq!(got[0].level, Level::Warn);
    assert!(
        got[0].detail.starts_with("NOT GUARDING"),
        "{}",
        got[0].detail
    );
}

#[test]
fn explicitly_recorded_off_is_reported_not_warned() {
    let got = guard_findings(
        None,
        &status("off", USER_CONFIG, "accepted"),
        Some(HOOKS_BOTH),
        &everywhere,
    );
    assert!(warnings(&got).is_empty(), "{got:#?}");
    assert!(got[0]
        .detail
        .starts_with(&format!("off by explicit choice recorded in {USER_CONFIG}")));
}

#[test]
fn unproven_quipu_auth_warns() {
    let got = guard_findings(
        None,
        &status("enforce", USER_CONFIG, "configured_acceptance_unproven"),
        Some(HOOKS_BOTH),
        &everywhere,
    );
    let warned = warnings(&got);
    assert_eq!(warned.len(), 1, "{got:#?}");
    assert_eq!(warned[0].subject, "yupana guard [worker] quipu auth");
    assert!(warned[0]
        .detail
        .starts_with("configured_acceptance_unproven"));
}

#[test]
fn present_codex_with_zero_hooks_warns() {
    let got = guard_findings(
        None,
        &status("enforce", USER_CONFIG, "accepted"),
        Some(HOOKS_CODEX_ZERO),
        &everywhere,
    );
    let warned = warnings(&got);
    assert_eq!(warned.len(), 1, "{got:#?}");
    assert_eq!(warned[0].subject, "yupana guard [worker] hooks codex");
    assert!(warned[0].detail.contains("0 yupana hook items"));
}

#[test]
fn absent_harness_with_zero_hooks_is_not_a_warning() {
    let got = guard_findings(
        None,
        &status("enforce", USER_CONFIG, "accepted"),
        Some(HOOKS_CODEX_ZERO),
        &|h: &str| h == "claude",
    );
    assert!(warnings(&got).is_empty(), "{got:#?}");
}

#[test]
fn unparseable_output_is_unknown_not_healthy() {
    for bad in ["policy mode: off\n", "{\"tenant\": \"worker\"}", ""] {
        let got = guard_findings(Some("worker"), bad, Some(HOOKS_BOTH), &everywhere);
        assert_eq!(got.len(), 1, "{got:#?}");
        assert_eq!(got[0].level, Level::Warn);
        assert!(got[0].detail.starts_with("UNKNOWN"), "{}", got[0].detail);
    }
    let got = guard_findings(
        None,
        &status("enforce", USER_CONFIG, "accepted"),
        Some("claude hooks are fine"),
        &everywhere,
    );
    let warned = warnings(&got);
    assert_eq!(warned.len(), 1);
    assert!(warned[0].detail.starts_with("UNKNOWN"));
}

#[test]
fn hooks_status_parses_both_shapes() {
    let st = parse_hooks_status(HOOKS_BOTH).unwrap();
    assert_eq!(
        st[0],
        HarnessHooks {
            harness: "claude".into(),
            items: 198,
            counts: Some((198, 30, 20))
        }
    );
    let direct = parse_hooks_status("claude: 0/6 yupana hook(s) in /h/.claude/settings.json\ncodex: 2/2 yupana hook(s) in /h/.codex/config.toml\n").unwrap();
    assert_eq!(direct[0].items, 0);
    assert_eq!(direct[1].items, 2);
    assert!(parse_hooks_status("").is_none());
    assert!(parse_hooks_status("claude: 3 hook item(s) via st\nnonsense\n").is_none());
}

#[test]
fn tenants_default_to_yupanas_own() {
    assert_eq!(tenants_from_env(None), vec![None]);
    assert_eq!(tenants_from_env(Some(" , ")), vec![None]);
    assert_eq!(
        tenants_from_env(Some("worker, sattler")),
        vec![Some("worker".to_owned()), Some("sattler".to_owned())]
    );
}

#[test]
fn tambo_auth_proof_is_a_host_line_and_never_rewrites_a_tenant_line() {
    let proven = host_auth_finding(&Ok("http://quipu.svc, credential file".to_owned()));
    assert_eq!(proven.subject, HOST_AUTH_SUBJECT);
    assert_eq!(proven.level, Level::Ok, "{proven:#?}");
    assert!(
        proven.detail.starts_with("proven at host level:"),
        "{}",
        proven.detail
    );
    assert!(proven.detail.contains("not a tenant that resolves another"));

    let failed = host_auth_finding(&Err("HTTP 401".to_owned()));
    assert_eq!(failed.level, Level::Warn);
    assert!(failed.detail.contains("UNPROVEN") && failed.detail.contains("HTTP 401"));

    // The tenant's own line is unchanged by any proof: still warn, still
    // yupana's own state.
    let tenant = guard_findings(
        Some("worker"),
        &status("enforce", USER_CONFIG, "configured_acceptance_unproven"),
        Some(HOOKS_BOTH),
        &everywhere,
    );
    let auth = tenant
        .iter()
        .find(|f| f.subject.ends_with(" quipu auth"))
        .unwrap();
    assert_eq!(auth.level, Level::Warn);
    assert!(auth.detail.starts_with("configured_acceptance_unproven"));
}
