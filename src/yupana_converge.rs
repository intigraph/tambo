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
    fs,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use toml_edit::{value, DocumentMut, Item, Table};

use crate::model::{Plan, ToolName, YupanaGuard, YupanaMode};

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
    policy.insert("mode", value(guard.mode.as_str()));
    if let Some(mut key) = policy.key_mut("mode") {
        key.leaf_decor_mut().set_prefix(mode_comment(guard));
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

fn yupana(args: &[&str]) -> Result<String> {
    let out = Command::new("yupana")
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

/// Converge Yupana to the plan's guard posture and print what happened.
/// A plan without `[yupana_guard]` changes nothing and says so.
pub fn converge(plan: &Plan) -> Result<()> {
    if !plan.tools.contains(&ToolName::Yupana) {
        return Ok(());
    }
    let Some(guard) = &plan.yupana_guard else {
        println!(
            "yupana guard: WARN the plan records no [yupana_guard]; policy mode, hooks and the Quipu \
             projection were NOT converged. Record `[yupana_guard] mode = \"advise\"|\"enforce\"` (or \
             \"off\" with off_reason) in the reviewed plan; `tambo doctor` reports the current state"
        );
        return Ok(());
    };
    let path = std::env::var_os("PATH");
    let on_path = |program: &str| !crate::adapter::which_all(program, path.as_deref()).is_empty();
    let mut failures = Vec::new();

    let config = crate::configuration::default_path()?;
    write_mode(&config, guard)?;
    match guard.off_reason.as_deref() {
        Some(reason) => println!(
            "yupana policy mode: off, recorded in {} (reason: {reason})",
            config.display()
        ),
        None => println!(
            "yupana policy mode: {} recorded in {}",
            guard.mode.as_str(),
            config.display()
        ),
    }

    let invocations = hook_install_invocations(on_path("st"), on_path("claude"), on_path("codex"));
    if invocations.is_empty() {
        println!("yupana hooks: no harness (claude, codex) on PATH; none installed");
    }
    for args in invocations {
        match yupana(&args) {
            Ok(_) => println!("yupana {}: ok", args.join(" ")),
            Err(error) => failures.push(format!("{error:#}")),
        }
    }

    match yupana(&["refresh-projection"]) {
        Ok(_) => println!("yupana refresh-projection: ok"),
        Err(error) => failures.push(format!("{error:#}")),
    }

    match configured_quipu_endpoint() {
        None => failures.push(
            "yupana quipu auth: UNPROVEN, no [yupana.quipu].endpoint in the bobbin config and no QUIPU_SERVER"
                .to_owned(),
        ),
        Some(endpoint) => match crate::quipu_credential::verify(&endpoint) {
            Ok(source) => println!("yupana quipu auth: PROVEN at {endpoint} ({source})"),
            Err(error) => failures.push(format!("yupana quipu auth: UNPROVEN at {endpoint}: {error:#}")),
        },
    }

    for finding in crate::doctor::yupana_guard_findings(path.as_deref()) {
        let level = match finding.level {
            crate::doctor::Level::Ok => "ok",
            crate::doctor::Level::Warn => "WARN",
            crate::doctor::Level::Fail => "FAIL",
        };
        println!("{}: {level}: {}", finding.subject, finding.detail);
        if finding.level != crate::doctor::Level::Ok {
            failures.push(format!("{}: {}", finding.subject, finding.detail));
        }
    }

    if !failures.is_empty() {
        bail!(
            "Yupana is installed but not in the plan's guard state ({} problem(s)):\n{}\nRun `tambo doctor` for the full guard report.",
            failures.len(),
            failures.join("\n")
        );
    }
    println!("yupana guard: converged to mode {}", guard.mode.as_str());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
