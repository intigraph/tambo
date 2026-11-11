//! Yupana guard posture for `tambo doctor` (aegis-r1s).
//!
//! A host can have Yupana installed and its MCP connected while the governed
//! edit guard blocks nothing: policy mode `off` or `advise`, no scope for the
//! tenant, an unproven Quipu credential, or no hooks in a harness that is
//! present. Doctor used to report only the MCP registration, so such a host
//! read as healthy. These checks read Yupana's own reports (`yupana status
//! --json` and `yupana hooks status`) and never change them.
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::doctor::{Finding, Level};

/// Hook counts for one harness as `yupana hooks status` prints them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HarnessHooks {
    pub name: String,
    pub items: usize,
    pub configured: usize,
    /// `None` when the direct (non-st) form was printed: it has no live data.
    pub live: Option<usize>,
    pub firing: Option<usize>,
}

/// Parse both forms `yupana hooks status` prints, one harness per line:
/// `claude: 24 hook item(s) via st; configured ok 24, live ok 18, firing ok 9, silent 3`
/// `codex: 3/4 yupana hook(s) in /path/config.toml`
pub(crate) fn parse_hooks(text: &str) -> Vec<HarnessHooks> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((name, rest)) = line.trim().split_once(": ") else {
            continue;
        };
        let name = name.trim().to_owned();
        if let Some(rest) = rest.strip_suffix("").filter(|r| r.contains("hook item(s) via st")) {
            let number = |key: &str| -> Option<usize> {
                let at = rest.find(key)? + key.len();
                rest[at..]
                    .trim_start()
                    .split(|c: char| !c.is_ascii_digit())
                    .next()?
                    .parse()
                    .ok()
            };
            let items = rest.split_whitespace().next().and_then(|n| n.parse().ok());
            if let (Some(items), Some(configured)) = (items, number("configured ok")) {
                out.push(HarnessHooks {
                    name,
                    items,
                    configured,
                    live: number("live ok"),
                    firing: number("firing ok"),
                });
            }
        } else if let Some((counts, _)) = rest.split_once(" yupana hook(s)") {
            if let Some((have, want)) = counts.trim().split_once('/') {
                if let (Ok(have), Ok(want)) = (have.parse(), want.parse()) {
                    out.push(HarnessHooks {
                        name,
                        items: want,
                        configured: have,
                        live: None,
                        firing: None,
                    });
                }
            }
        }
    }
    out
}

/// What the doctor knows about Quipu credential acceptance for this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Credential {
    /// Tambo's authenticated `/shapes` read accepted the resolver-selected
    /// credential, which uses the same precedence Yupana uses.
    Accepted,
    Rejected,
    /// No remote Quipu is configured, so acceptance was not probed.
    NotProbed,
}

fn short_digest(digest: &str) -> &str {
    let hex = digest.strip_prefix("sha256:").unwrap_or(digest);
    &hex[..hex.len().min(16)]
}

/// Findings from `yupana status --json`, hooks text and the credential probe.
/// `present` names the harness CLIs found on PATH (`claude`, `codex`).
pub(crate) fn posture_findings(
    status: &Value,
    hooks: &[HarnessHooks],
    present: &[&str],
    credential: Credential,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let tenant = status["tenant"].as_str().unwrap_or("default");
    let subject = |what: &str| format!("yupana {what} [{tenant}]");

    // Policy mode and scope decide whether the guard can block anything.
    let mode = status["policy"]["mode"].as_str().unwrap_or("unknown");
    let scope = status["policy"]["scope"].as_str();
    let source = status["policy_mode_provenance"]["source"]
        .as_str()
        .unwrap_or("unknown source");
    let scope_text = scope.unwrap_or("none");
    findings.push(match (mode, scope) {
        ("enforce", Some(_)) => Finding::new(
            Level::Ok,
            subject("policy"),
            format!("mode=enforce scope={scope_text} (from {source})"),
        ),
        ("enforce", None) => Finding::new(
            Level::Warn,
            subject("policy"),
            format!("mode=enforce but scope=none for this tenant, so the edit guard blocks nothing (from {source})"),
        ),
        ("advise", _) => Finding::new(
            Level::Warn,
            subject("policy"),
            format!("mode=advise scope={scope_text}: rules only advise, the edit guard blocks nothing (from {source})"),
        ),
        ("off", _) => Finding::new(
            Level::Warn,
            subject("policy"),
            format!("mode=off scope={scope_text}: the edit guard is disabled (from {source})"),
        ),
        (other, _) => Finding::new(
            Level::Warn,
            subject("policy"),
            format!("mode={other} not recognised; guard posture unknown"),
        ),
    });

    // Rule set: what the guard would enforce, and whether it could load it.
    let rules = &status["rule_set"];
    let state = rules["state"].as_str().unwrap_or("unknown");
    let counts = format!(
        "{} projected ({} structural, {} text) + {} local, digest {}",
        rules["projected"].as_u64().unwrap_or(0),
        rules["structural"].as_u64().unwrap_or(0),
        rules["text"].as_u64().unwrap_or(0),
        rules["local"].as_u64().unwrap_or(0),
        short_digest(rules["digest"].as_str().unwrap_or("none")),
    );
    findings.push(match state {
        "loaded" if rules["projected"].as_u64().unwrap_or(0) + rules["local"].as_u64().unwrap_or(0) > 0 => {
            Finding::new(Level::Ok, subject("rules"), format!("loaded: {counts}"))
        }
        "loaded" | "empty" => Finding::new(Level::Warn, subject("rules"), format!("no rules to enforce: {counts}")),
        "stale" => Finding::new(Level::Warn, subject("rules"), format!("serving a stale cache: {counts}")),
        "degraded" => Finding::new(
            Level::Fail,
            subject("rules"),
            "rule projection failed with no cache: the guard fails open",
        ),
        "off" => Finding::new(Level::Warn, subject("rules"), "rule projection is off"),
        other => Finding::new(Level::Warn, subject("rules"), format!("state {other}; {counts}")),
    });

    let tiers: Vec<&str> = status["tiers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    findings.push(Finding::new(
        Level::Ok,
        subject("tiers"),
        if tiers.is_empty() { "none reported".to_owned() } else { tiers.join(", ") },
    ));

    findings.push(if status["signed_rule_set"]["loaded"].as_bool() == Some(true) {
        Finding::new(Level::Ok, subject("signed cache"), "rules verified from the signed resident cache")
    } else {
        Finding::new(
            Level::Warn,
            subject("signed cache"),
            format!(
                "absent ({}): rules are an unsigned live projection, trusted on transport alone",
                status["signed_rule_set"]["state"].as_str().unwrap_or("unknown")
            ),
        )
    });

    // Yupana itself never proves acceptance; it reports what it found.
    let auth = status["quipu"]["auth"]["state"].as_str().unwrap_or("unknown");
    findings.push(match (auth, credential) {
        ("configured_acceptance_unproven", Credential::Accepted) => Finding::new(
            Level::Ok,
            subject("quipu auth"),
            "proven: the credential Yupana resolves was accepted by an authenticated /shapes read",
        ),
        ("configured_acceptance_unproven", Credential::Rejected) => Finding::new(
            Level::Fail,
            subject("quipu auth"),
            "the credential Yupana resolves was rejected; see `quipu credential` above",
        ),
        ("configured_acceptance_unproven", Credential::NotProbed) => Finding::new(
            Level::Warn,
            subject("quipu auth"),
            "credential found but acceptance unproven: no remote Quipu is configured to probe",
        ),
        ("writes_disabled", _) => Finding::new(
            Level::Warn,
            subject("quipu auth"),
            "writes disabled for this session after an earlier credential refusal",
        ),
        (other, _) => Finding::new(
            Level::Fail,
            subject("quipu auth"),
            format!("{other}: Yupana cannot authenticate to Quipu; install the issued token at ~/.config/quipu/token"),
        ),
    });

    // Hooks: a harness that is present must carry Yupana's hooks.
    for &harness in present {
        let found = hooks.iter().find(|h| h.name == harness);
        findings.push(match found {
            None => Finding::new(
                Level::Warn,
                format!("yupana hooks {harness}"),
                "not reported by `yupana hooks status`; hook state unknown",
            ),
            Some(h) if h.items == 0 || h.configured == 0 => Finding::new(
                Level::Fail,
                format!("yupana hooks {harness}"),
                format!("{harness} is installed but has no Yupana hooks, so its edits are never guarded; run `yupana hooks install`"),
            ),
            Some(h) if h.configured < h.items => Finding::new(
                Level::Warn,
                format!("yupana hooks {harness}"),
                format!("{}/{} configured", h.configured, h.items),
            ),
            Some(h) => {
                let live = h.live.map_or("n/a".to_owned(), |n| n.to_string());
                let firing = h.firing.map_or("n/a".to_owned(), |n| n.to_string());
                let detail = format!("configured {}/{}, live {live}, firing {firing}", h.configured, h.items);
                if h.live == Some(0) {
                    Finding::new(
                        Level::Warn,
                        format!("yupana hooks {harness}"),
                        format!("{detail}: configured but no live session loads them"),
                    )
                } else {
                    Finding::new(Level::Ok, format!("yupana hooks {harness}"), detail)
                }
            }
        });
    }
    findings
}

fn run(args: &[&str]) -> Option<(i32, String)> {
    let out = Command::new("yupana")
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    Some((out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// Live posture: runs Yupana's read-only reports. `present` lists harness
/// CLIs on PATH. Missing or unparseable output is reported, never assumed.
pub(crate) fn check(present: &[&str], credential: Credential) -> Vec<Finding> {
    // `yupana status` exits 3 when the rule set is degraded; its JSON is still valid.
    let status = match run(&["status", "--json"]) {
        Some((_, text)) => match serde_json::from_str::<Value>(&text) {
            Ok(value) => value,
            Err(_) => {
                return vec![Finding::new(
                    Level::Warn,
                    "yupana posture",
                    "`yupana status --json` did not return JSON; guard posture unknown",
                )]
            }
        },
        None => {
            return vec![Finding::new(
                Level::Warn,
                "yupana posture",
                "could not run `yupana status --json`; guard posture unknown",
            )]
        }
    };
    let hooks = run(&["hooks", "status"]).map(|(_, text)| parse_hooks(&text)).unwrap_or_default();
    posture_findings(&status, &hooks, present, credential)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Captured verbatim on a crew host 2026-11-11 (yupana 0.18.2, tenant worker).
    const HOST_STATUS: &str = r#"{"base_commit":null,"base_ref":"main","languages":["rust"],
      "metrics":{"record_paths":"off"},
      "policy":{"enforcing_without_scope":false,"mode":"advise","scope":null,"scope_configured":false},
      "policy_mode_provenance":{"effective":"advise","lowered_by_project":false,"source":"/home/u/.config/bobbin/config.toml","user_mode":"advise"},
      "quipu":{"auth":{"help":"h","session_scope":"harness_session","state":"configured_acceptance_unproven"},"branch_model":"qualifier","enabled":true},
      "rule_set":{"attempts":1,"cache_age_secs":null,"digest":"sha256:e8921d7d84522dea7135f54a88e9319cda10c2dea5448dd1a63911be90366868","error":null,"graph_enabled":true,"local":0,"projected":12,"state":"loaded","structural":0,"text":12,"verification":"unsigned"},
      "signed_rule_set":{"loaded":false,"note":"n","state":"never-loaded"},
      "tenant":"worker","tiers":["treesitter"]}"#;
    const HOST_HOOKS: &str = "claude: 24 hook item(s) via st; configured ok 24, live ok 18, firing ok 9, silent 3\n\
codex: 8 hook item(s) via st; configured ok 8, live ok 0, firing ok 0, silent 0\n";

    fn level(findings: &[Finding], subject: &str) -> Level {
        findings
            .iter()
            .find(|f| f.subject.starts_with(subject))
            .unwrap_or_else(|| panic!("no finding {subject}: {findings:?}"))
            .level
    }

    #[test]
    fn parses_both_hook_forms() {
        let parsed = parse_hooks(HOST_HOOKS);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], HarnessHooks { name: "claude".into(), items: 24, configured: 24, live: Some(18), firing: Some(9) });
        assert_eq!(parsed[1].live, Some(0));
        let direct = parse_hooks("codex: 3/4 yupana hook(s) in /h/.codex/config.toml\nnoise\n");
        assert_eq!(direct, vec![HarnessHooks { name: "codex".into(), items: 4, configured: 3, live: None, firing: None }]);
        assert!(parse_hooks("").is_empty());
    }

    #[test]
    fn crew_host_is_not_silent() {
        let status: Value = serde_json::from_str(HOST_STATUS).unwrap();
        let f = posture_findings(&status, &parse_hooks(HOST_HOOKS), &["claude", "codex"], Credential::Accepted);
        assert_eq!(level(&f, "yupana policy"), Level::Warn);
        assert!(f.iter().any(|x| x.detail.contains("blocks nothing")));
        assert_eq!(level(&f, "yupana rules"), Level::Ok);
        assert!(f.iter().any(|x| x.detail.contains("0 structural, 12 text") && x.detail.contains("e8921d7d84522dea")));
        assert_eq!(level(&f, "yupana tiers"), Level::Ok);
        assert_eq!(level(&f, "yupana signed cache"), Level::Warn);
        assert_eq!(level(&f, "yupana quipu auth"), Level::Ok);
        assert_eq!(level(&f, "yupana hooks claude"), Level::Ok);
        assert_eq!(level(&f, "yupana hooks codex"), Level::Warn);
        assert!(f.iter().all(|x| !x.subject.starts_with("yupana policy") || x.subject.contains("[worker]")));
    }

    #[test]
    fn mac_shape_flags_off_mode_unproven_auth_and_missing_codex_hooks() {
        let mut status: Value = serde_json::from_str(HOST_STATUS).unwrap();
        status["policy"]["mode"] = "off".into();
        let hooks = parse_hooks("claude: 6 hook item(s) via st; configured ok 6, live ok 6, firing ok 4, silent 0\ncodex: 0 hook item(s) via st; configured ok 0, live ok 0, firing ok 0, silent 0\n");
        let f = posture_findings(&status, &hooks, &["claude", "codex"], Credential::NotProbed);
        assert_eq!(level(&f, "yupana policy"), Level::Warn);
        assert!(f.iter().any(|x| x.detail.contains("mode=off")));
        assert_eq!(level(&f, "yupana quipu auth"), Level::Warn);
        assert_eq!(level(&f, "yupana hooks codex"), Level::Fail);
    }

    #[test]
    fn enforcing_with_scope_passes_and_failures_are_loud() {
        let mut status: Value = serde_json::from_str(HOST_STATUS).unwrap();
        status["policy"]["mode"] = "enforce".into();
        status["policy"]["scope"] = "crew".into();
        status["signed_rule_set"]["loaded"] = true.into();
        let f = posture_findings(&status, &parse_hooks(HOST_HOOKS), &["claude"], Credential::Accepted);
        assert_eq!(level(&f, "yupana policy"), Level::Ok);
        assert_eq!(level(&f, "yupana signed cache"), Level::Ok);
        status["policy"]["scope"] = Value::Null;
        status["rule_set"]["state"] = "degraded".into();
        status["quipu"]["auth"]["state"] = "missing".into();
        let f = posture_findings(&status, &[], &["claude"], Credential::Rejected);
        assert_eq!(level(&f, "yupana policy"), Level::Warn);
        assert_eq!(level(&f, "yupana rules"), Level::Fail);
        assert_eq!(level(&f, "yupana quipu auth"), Level::Fail);
        assert_eq!(level(&f, "yupana hooks claude"), Level::Warn);
        let f = posture_findings(&status, &[], &[], Credential::Rejected);
        assert!(f.iter().all(|x| !x.subject.starts_with("yupana hooks")));
    }
}
