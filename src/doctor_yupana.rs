//! Yupana guard state, per tenant (aegis-47ca, a slice of aegis-r1s).
//!
//! "registered and connected" says the MCP server answers; it does not say the
//! capability guard is doing anything. On 2026-11-11 a host printed exactly
//! that while `yupana status` showed policy mode `off` from the compiled
//! default, unproven Quipu auth, and Codex with zero hook items. This check
//! reads `yupana status --json` and `yupana hooks status` and says, per tenant,
//! whether the host is guarding. Nothing here changes yupana's configuration.
//!
//! Output it cannot read is UNKNOWN (a `warn` line that says so), never healthy.

use std::process::{Command, Stdio};

use serde_json::Value;

use super::{Finding, Level};

/// Policy-mode source yupana reports when no config layer set the mode.
const COMPILED_DEFAULT: &str = "compiled default";

/// Tenants to inspect: `TAMBO_YUPANA_TENANTS` (comma-separated) when set,
/// else yupana's own default tenant.
pub(super) fn tenants_from_env(value: Option<&str>) -> Vec<Option<String>> {
    let named: Vec<Option<String>> = value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|tenant| !tenant.is_empty())
        .map(|tenant| Some(tenant.to_owned()))
        .collect();
    if named.is_empty() {
        vec![None]
    } else {
        named
    }
}

/// One harness line of `yupana hooks status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HarnessHooks {
    pub harness: String,
    pub items: u64,
    /// `configured ok / live ok / firing ok`, when st reports them.
    pub counts: Option<(u64, u64, u64)>,
}

fn number_before(text: &str, marker: &str) -> Option<u64> {
    let head = text.split(marker).next()?;
    head.split_whitespace().last()?.parse().ok()
}

fn number_after(text: &str, marker: &str) -> Option<u64> {
    let (_, tail) = text.split_once(marker)?;
    tail.trim_start()
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// Parse `yupana hooks status`. Two shapes exist (yupana 0.19):
/// `claude: 198 hook item(s) via st; configured ok 198, live ok 30, firing ok 20, silent 0`
/// and, without st, `codex: 0/2 yupana hook(s) in ~/.codex/config.toml`.
/// `None` when any non-blank line matches neither: a partial read is UNKNOWN.
pub(super) fn parse_hooks_status(text: &str) -> Option<Vec<HarnessHooks>> {
    let mut harnesses = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let (harness, rest) = line.split_once(": ")?;
        if harness.is_empty() || harness.contains(' ') {
            return None;
        }
        let entry = if rest.contains("hook item(s)") {
            let counts = match (
                number_after(rest, "configured ok"),
                number_after(rest, "live ok"),
                number_after(rest, "firing ok"),
            ) {
                (Some(c), Some(l), Some(f)) => Some((c, l, f)),
                _ => None,
            };
            HarnessHooks {
                harness: harness.to_owned(),
                items: number_before(rest, "hook item(s)")?,
                counts,
            }
        } else if rest.contains("yupana hook(s)") {
            let fraction = rest.split_whitespace().next()?;
            let (have, _want) = fraction.split_once('/')?;
            HarnessHooks {
                harness: harness.to_owned(),
                items: have.parse().ok()?,
                counts: None,
            }
        } else {
            return None;
        };
        harnesses.push(entry);
    }
    (!harnesses.is_empty()).then_some(harnesses)
}

fn text<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    value.pointer(pointer).and_then(Value::as_str)
}

/// Turn one tenant's status JSON and hooks text into findings. `present`
/// says whether a harness's CLI is on this host; zero hooks there is a warning.
pub(super) fn guard_findings(
    tenant: Option<&str>,
    status: &str,
    hooks: Option<&str>,
    present: &dyn Fn(&str) -> bool,
) -> Vec<Finding> {
    let label = |name: &str| match name {
        "" => "yupana guard".to_owned(),
        name => format!("yupana guard [{name}]"),
    };
    let Ok(json) = serde_json::from_str::<Value>(status) else {
        return vec![unknown(
            &label(tenant.unwrap_or_default()),
            "`yupana status --json` output is not JSON",
        )];
    };
    let tenant = text(&json, "/tenant")
        .or(tenant)
        .unwrap_or_default()
        .to_owned();
    let subject = label(&tenant);
    let mode =
        text(&json, "/policy_mode_provenance/effective").or_else(|| text(&json, "/policy/mode"));
    let source = text(&json, "/policy_mode_provenance/source");
    let auth = text(&json, "/quipu/auth/state");
    let (Some(mode), Some(auth)) = (mode, auth) else {
        return vec![unknown(
            &subject,
            "`yupana status --json` has no policy mode or quipu auth state",
        )];
    };
    let source = source.unwrap_or("unreported");
    let scope = match json.pointer("/policy/scope") {
        Some(Value::String(scope)) => scope.clone(),
        Some(Value::Null) | None => "none".to_owned(),
        Some(other) => other.to_string(),
    };
    let rules = match (
        json.pointer("/rule_set/projected").and_then(Value::as_u64),
        text(&json, "/rule_set/digest"),
    ) {
        (Some(count), Some(digest)) => format!("{count} projected rule(s), digest {digest}"),
        (Some(count), None) => format!("{count} projected rule(s), no digest"),
        _ => "projected rules unreported".to_owned(),
    };
    let signed = match json
        .pointer("/signed_rule_set/loaded")
        .and_then(Value::as_bool)
    {
        Some(true) => "signed resident cache present",
        Some(false) => "signed resident cache absent",
        None => "signed resident cache unreported",
    };

    let mut findings = Vec::new();
    let summary = format!(
        "policy mode {mode} (source: {source}), scope {scope}, {rules}, quipu auth {auth}, {signed}"
    );
    findings.push(match mode {
        "off" if source == COMPILED_DEFAULT => Finding::new(
            Level::Warn,
            subject.clone(),
            format!("NOT GUARDING: {summary}. Mode is off only because nothing set it; record a mode in the bobbin config (`[yupana.policy] mode = ...`) or record off explicitly"),
        ),
        "off" => Finding::new(
            Level::Ok,
            subject.clone(),
            format!("off by explicit choice recorded in {source}: {summary}"),
        ),
        _ => Finding::new(Level::Ok, subject.clone(), summary),
    });
    if json
        .pointer("/policy/enforcing_without_scope")
        .and_then(Value::as_bool)
        == Some(true)
    {
        findings.push(Finding::new(
            Level::Warn,
            format!("{subject} scope"),
            "enforcing with no scope configured; the guard has nothing to enforce against",
        ));
    }
    if auth != "accepted" {
        findings.push(Finding::new(
            Level::Warn,
            format!("{subject} quipu auth"),
            format!("{auth}: yupana has not proven Quipu accepts its credential, so guard verdicts may not reach the graph"),
        ));
    }
    if mode != "off" {
        if let Some(state) = text(&json, "/rule_set/state").filter(|state| *state != "loaded") {
            findings.push(Finding::new(
                Level::Warn,
                format!("{subject} rules"),
                format!("rule set is {state} while policy mode is {mode}"),
            ));
        }
    }

    match hooks.map(parse_hooks_status) {
        None => findings.push(unknown(
            &format!("{subject} hooks"),
            "`yupana hooks status` did not run",
        )),
        Some(None) => findings.push(unknown(
            &format!("{subject} hooks"),
            "`yupana hooks status` output not understood",
        )),
        Some(Some(harnesses)) => {
            for hooks in harnesses {
                let name = &hooks.harness;
                let counts = hooks.counts.map_or(String::new(), |(c, l, f)| {
                    format!(" (configured {c}, live {l}, firing {f})")
                });
                let finding = if !present(name) {
                    Finding::new(
                        Level::Ok,
                        format!("{subject} hooks {name}"),
                        format!(
                            "{} hook item(s){counts}; {name} is not on PATH",
                            hooks.items
                        ),
                    )
                } else if hooks.items == 0 {
                    Finding::new(
                        Level::Warn,
                        format!("{subject} hooks {name}"),
                        format!("{name} is present with 0 yupana hook items; it runs unguarded. Install them with `yupana hooks install --harness {name}`"),
                    )
                } else {
                    Finding::new(
                        Level::Ok,
                        format!("{subject} hooks {name}"),
                        format!("{} hook item(s){counts}", hooks.items),
                    )
                };
                findings.push(finding);
            }
        }
    }
    findings
}

fn unknown(subject: &str, why: &str) -> Finding {
    Finding::new(
        Level::Warn,
        subject,
        format!("UNKNOWN (not healthy): {why}; guard state cannot be confirmed"),
    )
}

fn run(args: &[&str], tenant: Option<&str>) -> Option<String> {
    let mut command = Command::new("yupana");
    command
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    if let Some(tenant) = tenant {
        command.args(["--tenant", tenant]);
    }
    let out = command.output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Subject of Tambo's own Quipu credential proof. It is a HOST-level line:
/// one authenticated read with the credential this process resolves
/// (QUIPU_AUTH_TOKEN > QUIPU_AUTH_TOKEN_FILE > ~/.config/quipu/token) against
/// the endpoint the bobbin config names. It does not prove any tenant whose
/// yupana resolves another credential or endpoint, so it never rewrites a
/// tenant's own `quipu auth` line.
pub(crate) const HOST_AUTH_SUBJECT: &str = "yupana quipu auth (host)";

/// `yupana status` never claims acceptance (it reports at best
/// `configured_acceptance_unproven`). Tambo's host-level proof is reported as
/// its own finding beside the tenant lines, which stay `warn`.
pub(super) fn host_auth_finding(proof: &Result<String, String>) -> Finding {
    match proof {
        Ok(how) => Finding::new(
            Level::Ok,
            HOST_AUTH_SUBJECT,
            format!(
                "proven at host level: Tambo's authenticated Quipu read was accepted ({how}); \
                 this covers this host's credential and endpoint only, not a tenant that resolves another"
            ),
        ),
        Err(why) => Finding::new(
            Level::Warn,
            HOST_AUTH_SUBJECT,
            format!("UNPROVEN: Tambo's authenticated Quipu read failed: {why}"),
        ),
    }
}

fn auth_proof() -> Result<String, String> {
    let endpoint = crate::yupana_converge::configured_quipu_endpoint().ok_or_else(|| {
        "no [yupana.quipu].endpoint in the bobbin config and no QUIPU_SERVER".to_owned()
    })?;
    crate::quipu_credential::verify(&endpoint)
        .map(|source| format!("{endpoint}, credential {source}"))
        .map_err(|error| format!("{endpoint}: {error:#}"))
}

/// Live check: run yupana for each tenant, then (when any tenant's auth is
/// unproven) add Tambo's host-level proof as a separate line.
pub(super) fn check(path: Option<&std::ffi::OsStr>) -> Vec<Finding> {
    let mut findings = check_tenants(path);
    if findings
        .iter()
        .any(|f| f.level == Level::Warn && f.subject.ends_with(" quipu auth"))
    {
        findings.push(host_auth_finding(&auth_proof()));
    }
    findings
}

fn check_tenants(path: Option<&std::ffi::OsStr>) -> Vec<Finding> {
    if crate::adapter::which_all("yupana", path).is_empty() {
        return vec![unknown("yupana guard", "yupana is not on PATH")];
    }
    let present = |harness: &str| !crate::adapter::which_all(harness, path).is_empty();
    tenants_from_env(std::env::var("TAMBO_YUPANA_TENANTS").ok().as_deref())
        .into_iter()
        .flat_map(|tenant| {
            let tenant = tenant.as_deref();
            match run(&["status", "--json"], tenant) {
                None => vec![unknown(
                    &format!(
                        "yupana guard{}",
                        tenant.map_or(String::new(), |t| format!(" [{t}]"))
                    ),
                    "`yupana status --json` failed",
                )],
                Some(status) => {
                    let hooks = run(&["hooks", "status"], tenant);
                    guard_findings(tenant, &status, hooks.as_deref(), &present)
                }
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "doctor_yupana_tests.rs"]
mod tests;
