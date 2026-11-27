//! Leave Yupana guarding the way the plan says (aegis-r1s WANT 1).
//!
//! Installing the binary is not installing the guard. On 2026-11-11 a host had
//! Yupana present while its policy mode was `off` from the compiled default,
//! Codex had zero hook items and the Quipu credential was unproven. When the
//! plan records a `[yupana_guard]`, `tambo install` and `tambo update`
//! converge each of those: the mode is written to the bobbin config, hooks are
//! installed for every harness present, the Quipu policy projection is
//! refreshed, the credential is proven against Quipu, and the guard is read
//! back through the same check `tambo doctor` runs. Anything short of that
//! fails the command loudly instead of reporting success.
//!
//! Which rules exist and which scopes apply stay the deployment's choice;
//! nothing here writes rules or scopes.

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use toml_edit::{value, DocumentMut, Item, Table};

use crate::{
    doctor::{Finding, Level},
    model::{Plan, ToolName, YupanaGuard, YupanaMode},
};

/// Comment Tambo writes above the mode it owns, so a reader of the bobbin
/// config can see where the value came from and why.
fn mode_comment(guard: &YupanaGuard) -> String {
    match (guard.mode, guard.off_reason.as_deref()) {
        (YupanaMode::Off, Some(reason)) => format!(
            "# tambo: [yupana_guard] in the reviewed plan records mode off: {}\n",
            reason.trim().replace('\n', " ")
        ),
        _ => "# tambo: mode set from [yupana_guard] in the reviewed plan (aegis-r1s)\n".to_owned(),
    }
}

/// Write `[yupana.policy].mode` (and only that key), keeping everything else in
/// the file, then read it back.
pub fn write_mode(path: &Path, guard: &YupanaGuard) -> Result<()> {
    let mut document = if path.exists() {
        fs::read_to_string(path)
            .with_context(|| format!("read bobbin config {}", path.display()))?
            .parse::<DocumentMut>()
            .with_context(|| {
                format!(
                    "parse bobbin config {}; refusing to overwrite it",
                    path.display()
                )
            })?
    } else {
        DocumentMut::new()
    };
    for (parent, key) in [(None, "yupana"), (Some("yupana"), "policy")] {
        let table = match parent {
            None => document.as_table_mut(),
            Some(parent) => document[parent]
                .as_table_mut()
                .with_context(|| format!("[{parent}] must be a TOML table"))?,
        };
        if table.get(key).is_none() {
            let mut fresh = Table::new();
            fresh.set_implicit(parent.is_none());
            table.insert(key, Item::Table(fresh));
        }
        table[key].as_table_mut().with_context(|| {
            format!(
                "[{}{key}] must be a TOML table",
                parent.map_or(String::new(), |p| format!("{p}."))
            )
        })?;
    }
    let policy = document["yupana"]["policy"]
        .as_table_mut()
        .context("[yupana.policy] must be a TOML table")?;
    // Keep any operator comment above `mode`; replace only the line Tambo owns.
    let kept: String = policy
        .key("mode")
        .and_then(|key| key.leaf_decor().prefix())
        .and_then(|prefix| prefix.as_str())
        .unwrap_or_default()
        .split_inclusive('\n')
        .filter(|line| !line.trim_start().starts_with("# tambo:"))
        .collect();
    policy.insert("mode", value(guard.mode.as_str()));
    if let Some(mut key) = policy.key_mut("mode") {
        key.leaf_decor_mut()
            .set_prefix(format!("{kept}{}", mode_comment(guard)));
    }

    let parent = path.parent().context("bobbin config path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let temp = path.with_extension("toml.tambo-tmp");
    fs::write(&temp, document.to_string()).with_context(|| format!("write {}", temp.display()))?;
    fs::rename(&temp, path).with_context(|| format!("atomically replace {}", path.display()))?;

    let read_back: toml::Value = toml::from_str(&fs::read_to_string(path)?)
        .with_context(|| format!("re-read {}", path.display()))?;
    let got = read_back
        .get("yupana")
        .and_then(|y| y.get("policy"))
        .and_then(|p| p.get("mode"))
        .and_then(toml::Value::as_str);
    if got != Some(guard.mode.as_str()) {
        bail!(
            "bobbin config read-back: [yupana.policy].mode is {got:?}, expected {:?}",
            guard.mode.as_str()
        );
    }
    Ok(())
}

/// The Quipu endpoint Yupana is configured to use: `[yupana.quipu].endpoint`
/// in the bobbin config, else `QUIPU_SERVER`.
pub fn quipu_endpoint(config: Option<&str>, env_server: Option<&str>) -> Option<String> {
    config
        .and_then(|body| toml::from_str::<toml::Value>(body).ok())
        .and_then(|doc| {
            doc.get("yupana")?
                .get("quipu")?
                .get("endpoint")?
                .as_str()
                .map(str::to_owned)
        })
        .or_else(|| env_server.map(str::to_owned))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

pub(crate) fn configured_quipu_endpoint() -> Option<String> {
    let body = crate::configuration::default_path()
        .ok()
        .and_then(|path| fs::read_to_string(path).ok());
    quipu_endpoint(
        body.as_deref(),
        std::env::var("QUIPU_SERVER").ok().as_deref(),
    )
}

/// The `yupana hooks install` invocations for this host. With st present,
/// one install registers yupana's bundle and st renders it into every role's
/// Claude AND Codex settings; without st, each present harness is written
/// directly.
pub fn hook_install_invocations(st: bool, claude: bool, codex: bool) -> Vec<Vec<&'static str>> {
    if st {
        return vec![vec!["hooks", "install"]];
    }
    [("claude", claude), ("codex", codex)]
        .into_iter()
        .filter(|(_, present)| *present)
        .map(|(harness, _)| vec!["hooks", "install", "--no-st", "--harness", harness])
        .collect()
}

fn run_yupana(program: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("run `yupana {}`", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "`yupana {}` failed ({}): {}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// What convergence needs from the host, so the decision logic is testable
/// without touching the real HOME, PATH or Quipu.
pub(crate) trait Host {
    fn config_path(&self) -> Result<PathBuf>;
    fn on_path(&self, program: &str) -> bool;
    fn yupana(&self, args: &[&str]) -> Result<String>;
    /// The read-back: the same findings `tambo doctor` prints.
    fn guard_findings(&self) -> Vec<Finding>;
}

struct LiveHost {
    path: Option<OsString>,
}

impl Host for LiveHost {
    fn config_path(&self) -> Result<PathBuf> {
        crate::configuration::default_path()
    }
    fn on_path(&self, program: &str) -> bool {
        !crate::adapter::which_all(program, self.path.as_deref()).is_empty()
    }
    fn yupana(&self, args: &[&str]) -> Result<String> {
        run_yupana(Path::new("yupana"), args)
    }
    fn guard_findings(&self) -> Vec<Finding> {
        crate::doctor::yupana_guard_findings(self.path.as_deref())
    }
}

/// Converge Yupana to the plan's guard posture and print what happened.
/// A plan without `[yupana_guard]` changes nothing and says so.
pub fn converge(plan: &Plan) -> Result<()> {
    let host = LiveHost {
        path: std::env::var_os("PATH"),
    };
    for line in converge_on(plan, &host)? {
        println!("{line}");
    }
    Ok(())
}

/// The convergence decision. Returns the report lines on success; any
/// shortfall is an error naming every problem (the lines printed so far are
/// part of it). The mode is written first and is NOT rolled back when a later
/// step fails: the plan's mode is the intended state, and the error says the
/// rest of the guard is not there yet.
pub(crate) fn converge_on(plan: &Plan, host: &dyn Host) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    if !plan.tools.contains(&ToolName::Yupana) {
        return Ok(lines);
    }
    let Some(guard) = &plan.yupana_guard else {
        lines.push(
            "yupana guard: WARN the plan records no [yupana_guard]; policy mode, hooks and the Quipu \
             projection were NOT converged. Record `[yupana_guard] mode = \"advise\"|\"enforce\"` (or \
             \"off\" with off_reason) in the reviewed plan; `tambo doctor` reports the current state"
                .to_owned(),
        );
        return Ok(lines);
    };
    let mut failures = Vec::new();

    let config = host.config_path()?;
    write_mode(&config, guard)?;
    lines.push(match guard.off_reason.as_deref() {
        Some(reason) => format!(
            "yupana policy mode: off, recorded in {} (reason: {reason})",
            config.display()
        ),
        None => format!(
            "yupana policy mode: {} recorded in {}",
            guard.mode.as_str(),
            config.display()
        ),
    });

    let invocations = hook_install_invocations(
        host.on_path("st"),
        host.on_path("claude"),
        host.on_path("codex"),
    );
    if invocations.is_empty() {
        lines.push("yupana hooks: no harness (claude, codex) on PATH; none installed".to_owned());
    }
    for args in invocations {
        match host.yupana(&args) {
            Ok(_) => lines.push(format!("yupana {}: ok", args.join(" "))),
            Err(error) => failures.push(format!("{error:#}")),
        }
    }

    match host.yupana(&["refresh-projection"]) {
        Ok(_) => lines.push("yupana refresh-projection: ok".to_owned()),
        Err(error) => failures.push(format!("{error:#}")),
    }

    // Read back through doctor's own check. That check also carries Tambo's
    // credential proof, once, as a separate host-level line. Yupana itself
    // never reports a tenant's auth as accepted, so a tenant's `quipu auth`
    // warn is tolerated only beside a proven host line, and is still printed
    // as WARN so nobody reads it as per-tenant proof.
    let findings = host.guard_findings();
    let host_proven = findings
        .iter()
        .any(|f| f.subject == crate::doctor::YUPANA_HOST_AUTH && f.level == Level::Ok);
    for finding in findings {
        let level = match finding.level {
            Level::Ok => "ok",
            Level::Warn => "WARN",
            Level::Fail => "FAIL",
        };
        let tolerated =
            host_proven && finding.level == Level::Warn && finding.subject.ends_with(" quipu auth");
        let note = if tolerated {
            " (not blocking: proven at host level only; yupana cannot attest per-tenant acceptance)"
        } else {
            ""
        };
        lines.push(format!(
            "{}: {level}: {}{note}",
            finding.subject, finding.detail
        ));
        if finding.level != Level::Ok && !tolerated {
            failures.push(format!("{}: {}", finding.subject, finding.detail));
        }
    }

    if !failures.is_empty() {
        bail!(
            "{}\nYupana is installed but not in the plan's guard state ({} problem(s)); \
             [yupana.policy].mode is already written as {}:\n{}\nRun `tambo doctor` for the full guard report.",
            lines.join("\n"),
            failures.len(),
            guard.mode.as_str(),
            failures.join("\n")
        );
    }
    lines.push(format!(
        "yupana guard: converged to mode {}",
        guard.mode.as_str()
    ));
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Profile;
    use std::cell::RefCell;

    struct FakeHost {
        config: PathBuf,
        present: Vec<&'static str>,
        failing: Vec<&'static str>,
        findings: Vec<Finding>,
        calls: RefCell<Vec<String>>,
    }

    impl Host for FakeHost {
        fn config_path(&self) -> Result<PathBuf> {
            Ok(self.config.clone())
        }
        fn on_path(&self, program: &str) -> bool {
            self.present.contains(&program)
        }
        fn yupana(&self, args: &[&str]) -> Result<String> {
            let call = args.join(" ");
            self.calls.borrow_mut().push(call.clone());
            if self.failing.iter().any(|f| call.contains(f)) {
                bail!("`yupana {call}` failed (exit status: 1): boom");
            }
            Ok(String::new())
        }
        fn guard_findings(&self) -> Vec<Finding> {
            self.findings.clone()
        }
    }

    fn finding(level: Level, subject: &str) -> Finding {
        Finding {
            level,
            subject: subject.to_owned(),
            detail: "detail".to_owned(),
        }
    }

    fn healthy() -> Vec<Finding> {
        vec![
            finding(Level::Ok, "yupana guard"),
            finding(Level::Warn, "yupana guard quipu auth"),
            finding(Level::Ok, "yupana guard hooks claude"),
            finding(Level::Ok, crate::doctor::YUPANA_HOST_AUTH),
        ]
    }

    fn host(root: &Path, findings: Vec<Finding>) -> FakeHost {
        FakeHost {
            config: root.join("bobbin/config.toml"),
            present: vec!["claude", "codex"],
            failing: Vec::new(),
            findings,
            calls: RefCell::new(Vec::new()),
        }
    }

    fn guarded(mode: YupanaMode) -> Plan {
        let mut plan = Plan::for_profile(Profile::Everything);
        assert!(plan.tools.contains(&ToolName::Yupana));
        plan.yupana_guard = Some(guard(mode, None));
        plan
    }

    #[test]
    fn converge_without_a_guard_only_warns_and_touches_nothing() {
        let root = tempfile::tempdir().unwrap();
        let fake = host(root.path(), Vec::new());
        let plan = Plan::for_profile(Profile::Everything);
        let lines = converge_on(&plan, &fake).unwrap();
        assert!(
            lines[0].contains("WARN the plan records no [yupana_guard]"),
            "{lines:?}"
        );
        assert!(fake.calls.borrow().is_empty());
        assert!(!fake.config.exists());
    }

    #[test]
    fn converge_succeeds_only_with_a_clean_read_back_and_a_proven_host() {
        let root = tempfile::tempdir().unwrap();
        let fake = host(root.path(), healthy());
        let lines = converge_on(&guarded(YupanaMode::Enforce), &fake).unwrap();
        assert_eq!(
            *fake.calls.borrow(),
            vec![
                "hooks install --no-st --harness claude",
                "hooks install --no-st --harness codex",
                "refresh-projection",
            ]
        );
        let body = fs::read_to_string(&fake.config).unwrap();
        assert!(body.contains("mode = \"enforce\""), "{body}");
        let auth = lines
            .iter()
            .find(|l| l.starts_with("yupana guard quipu auth"))
            .unwrap();
        assert!(
            auth.contains("WARN") && auth.contains("proven at host level only"),
            "{auth}"
        );
        assert_eq!(
            lines.last().unwrap(),
            "yupana guard: converged to mode enforce"
        );
    }

    #[test]
    fn unproven_host_auth_fails_convergence_and_tenant_auth_is_not_excused() {
        let root = tempfile::tempdir().unwrap();
        let mut findings = healthy();
        findings.pop();
        findings.push(finding(Level::Warn, crate::doctor::YUPANA_HOST_AUTH));
        let error = converge_on(&guarded(YupanaMode::Advise), &host(root.path(), findings))
            .unwrap_err()
            .to_string();
        assert!(error.contains("2 problem(s)"), "{error}");
        assert!(error.contains("yupana guard quipu auth: detail"), "{error}");
        assert!(
            error.contains("mode is already written as advise"),
            "{error}"
        );
    }

    #[test]
    fn any_doctor_warn_or_fail_and_any_hook_failure_fail_convergence() {
        for (level, subject) in [
            (Level::Warn, "yupana guard hooks codex"),
            (Level::Fail, "yupana guard"),
        ] {
            let root = tempfile::tempdir().unwrap();
            let mut findings = healthy();
            findings.push(finding(level, subject));
            let error = converge_on(&guarded(YupanaMode::Enforce), &host(root.path(), findings))
                .unwrap_err()
                .to_string();
            assert!(error.contains(&format!("{subject}: detail")), "{error}");
        }
        let root = tempfile::tempdir().unwrap();
        let mut fake = host(root.path(), healthy());
        fake.failing = vec!["--harness codex"];
        let error = converge_on(&guarded(YupanaMode::Enforce), &fake)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("hooks install --no-st --harness codex` failed"),
            "{error}"
        );
        // A later failure does not skip the remaining steps.
        assert!(fake
            .calls
            .borrow()
            .contains(&"refresh-projection".to_owned()));
    }

    #[test]
    fn st_present_installs_once_through_st() {
        let root = tempfile::tempdir().unwrap();
        let mut fake = host(root.path(), healthy());
        fake.present = vec!["st", "claude", "codex"];
        converge_on(&guarded(YupanaMode::Enforce), &fake).unwrap();
        assert_eq!(
            *fake.calls.borrow(),
            vec!["hooks install", "refresh-projection"]
        );
    }

    #[test]
    fn live_runner_surfaces_a_failing_yupana_and_its_stderr() {
        let root = tempfile::tempdir().unwrap();
        let fake = root.path().join("yupana");
        crate::test_exec::write_executable(
            &fake,
            "#!/bin/sh\nif [ \"$1\" = refresh-projection ]; then echo 'quipu 401' >&2; exit 3; fi\necho \"ran $*\"\n",
        );
        assert_eq!(
            run_yupana(&fake, &["hooks", "status"]).unwrap(),
            "ran hooks status"
        );
        let error = run_yupana(&fake, &["refresh-projection"])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("`yupana refresh-projection` failed") && error.contains("quipu 401"),
            "{error}"
        );
    }

    #[test]
    fn operator_comment_above_mode_survives_and_ours_is_replaced() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.toml");
        fs::write(
            &path,
            "[yupana.policy]\n# operator: keep advise until rules settle\nmode = \"advise\"\n",
        )
        .unwrap();
        write_mode(&path, &guard(YupanaMode::Enforce, None)).unwrap();
        write_mode(&path, &guard(YupanaMode::Enforce, None)).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(
            body.contains("# operator: keep advise until rules settle\n# tambo:"),
            "{body}"
        );
        assert_eq!(body.matches("# tambo:").count(), 1, "{body}");
    }

    fn guard(mode: YupanaMode, reason: Option<&str>) -> YupanaGuard {
        YupanaGuard {
            mode,
            off_reason: reason.map(str::to_owned),
        }
    }

    #[test]
    fn mode_is_written_with_provenance_and_other_keys_survive() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.toml");
        fs::write(
            &path,
            "# keep me\n[search]\nlimit = 7\n\n[yupana.policy]\nmode = \"advise\"\ndeadline_ms = 900\n\n[yupana.quipu]\nendpoint = \"http://q\"\n",
        )
        .unwrap();
        write_mode(&path, &guard(YupanaMode::Enforce, None)).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        let doc: toml::Value = toml::from_str(&body).unwrap();
        assert_eq!(doc["yupana"]["policy"]["mode"].as_str(), Some("enforce"));
        assert_eq!(
            doc["yupana"]["policy"]["deadline_ms"].as_integer(),
            Some(900)
        );
        assert_eq!(doc["search"]["limit"].as_integer(), Some(7));
        assert_eq!(
            doc["yupana"]["quipu"]["endpoint"].as_str(),
            Some("http://q")
        );
        assert!(body.contains("# keep me"));
        assert!(body.contains("# tambo: mode set from [yupana_guard]"));
    }

    #[test]
    fn missing_config_is_created_and_off_records_its_reason() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("nested/config.toml");
        write_mode(&path, &guard(YupanaMode::Off, Some("lab host, no agents"))).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        let doc: toml::Value = toml::from_str(&body).unwrap();
        assert_eq!(doc["yupana"]["policy"]["mode"].as_str(), Some("off"));
        assert!(
            body.contains("records mode off: lab host, no agents"),
            "{body}"
        );
        // Idempotent: a second write leaves one comment, not two.
        write_mode(&path, &guard(YupanaMode::Off, Some("lab host, no agents"))).unwrap();
        assert_eq!(
            fs::read_to_string(&path)
                .unwrap()
                .matches("# tambo:")
                .count(),
            1
        );
    }

    #[test]
    fn malformed_config_is_refused_not_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.toml");
        fs::write(&path, "[yupana\nbroken").unwrap();
        assert!(write_mode(&path, &guard(YupanaMode::Advise, None)).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "[yupana\nbroken");
        fs::write(&path, "yupana = 3\n").unwrap();
        assert!(write_mode(&path, &guard(YupanaMode::Advise, None)).is_err());
    }

    #[test]
    fn hooks_go_through_st_when_present_else_to_each_present_harness() {
        assert_eq!(
            hook_install_invocations(true, false, false),
            vec![vec!["hooks", "install"]]
        );
        assert_eq!(
            hook_install_invocations(false, true, true),
            vec![
                vec!["hooks", "install", "--no-st", "--harness", "claude"],
                vec!["hooks", "install", "--no-st", "--harness", "codex"],
            ]
        );
        assert_eq!(
            hook_install_invocations(false, false, true),
            vec![vec!["hooks", "install", "--no-st", "--harness", "codex"]]
        );
        assert!(hook_install_invocations(false, false, false).is_empty());
    }

    #[test]
    fn endpoint_prefers_yupana_config_then_environment() {
        let body = "[yupana.quipu]\nendpoint = \"http://quipu.svc\"\n";
        assert_eq!(
            quipu_endpoint(Some(body), Some("http://env")).as_deref(),
            Some("http://quipu.svc")
        );
        assert_eq!(
            quipu_endpoint(Some("[search]\n"), Some("http://env")).as_deref(),
            Some("http://env")
        );
        assert_eq!(quipu_endpoint(None, Some("  ")), None);
        assert_eq!(quipu_endpoint(Some("not toml ["), None), None);
    }
}
