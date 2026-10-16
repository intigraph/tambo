//! Package-manager receipts for stack tools (aegis-6f1).
//!
//! On a host whose stack is delivered by a package manager, that manager is
//! the only writer. On apt hosts that is the root unattended-upgrades lane;
//! automated caboodle runs never call apt or sudo. Caboodle still has to judge
//! such a tool, so it reads the package receipt (the package that owns the
//! binary PATH runs, and its version) and compares the upstream version with
//! the reviewed pin: current, ahead (never downgraded) or behind (the package
//! lane will deliver it).

use std::{env, process::Command};

use crate::model::ToolName;

/// What the package manager says about a tool's binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub manager: &'static str,
    pub package: String,
    /// The upstream version, with any Debian epoch and revision removed.
    pub version: String,
}

/// `[epoch:]upstream[-revision]` -> `upstream`. The stack's packages use no
/// epoch and either no revision or a packaging-only one (quipu 0.13.0-2 adds
/// its qp link), so the upstream part is what the reviewed pin names.
#[must_use]
pub fn upstream_version(debian: &str) -> &str {
    let without_epoch = debian.split_once(':').map_or(debian, |(_, rest)| rest);
    without_epoch
        .rsplit_once('-')
        .map_or(without_epoch, |(upstream, _)| upstream)
}

/// The program whose owner decides who manages `tool`, if it has a binary.
#[must_use]
pub fn primary_program(tool: ToolName) -> Option<String> {
    match tool {
        ToolName::Quipu => Some("quipu".into()),
        ToolName::Bobbin => Some("bobbin".into()),
        ToolName::Yupana => Some("yupana".into()),
        ToolName::DesirePath => Some("dp".into()),
        ToolName::Camayoc => None,
        ToolName::Member(name) => {
            crate::members::get(name).and_then(|m| m.programs.first().map(|p| p.name.clone()))
        }
    }
}

/// The apt receipt for the `program` PATH runs, when dpkg owns it.
#[must_use]
pub fn apt_receipt(program: &str) -> Option<Receipt> {
    let path = crate::adapter::which_all(program, env::var_os("PATH").as_deref())
        .into_iter()
        .next()?;
    let package = crate::apt_owned::owner(&path)?;
    let query = env::var_os("CABOODLE_DPKG_QUERY").unwrap_or_else(|| "dpkg-query".into());
    let out = Command::new(query)
        .args(["-W", "-f", "${Version}", &package])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if raw.is_empty() {
        return None;
    }
    Some(Receipt {
        manager: "apt",
        version: upstream_version(&raw).to_owned(),
        package,
    })
}

/// The Homebrew tap that publishes the stack (aegis-em2). A formula installed
/// from any other tap is package-owned but never driven by caboodle.
pub const STACK_TAP: &str = "scbrown/tap";

/// The stack formula for a member tool. caboodle-self is NOT here: on the Mac
/// its installer lane owns it until the caboodle cutover (aegis-6f1 (a)).
#[must_use]
pub fn brew_formula(tool: ToolName) -> Option<&'static str> {
    match tool {
        ToolName::Bobbin => Some("bobbin"),
        ToolName::DesirePath => Some("desire-path"),
        ToolName::Quipu => Some("quipu"),
        ToolName::Yupana => Some("yupana"),
        ToolName::Member("seeds") => Some("seeds"),
        // Chaski needs Python >= 3.11; its formula owns a python@3.12, which a
        // release-fallback wheel on macOS's /usr/bin/python3 (3.9) cannot (aegis-o7n).
        ToolName::Member("chaski") => Some("chaski"),
        _ => None,
    }
}

/// The brew executable, only where caboodle may drive it: macOS by default,
/// or any host that names one explicitly in CABOODLE_BREW (tests use a fake).
#[must_use]
pub fn brew_program() -> Option<std::ffi::OsString> {
    if let Some(explicit) = env::var_os("CABOODLE_BREW") {
        return Some(explicit);
    }
    cfg!(target_os = "macos").then(|| "brew".into())
}

/// `brew info --json=v2 <tap>/<formula>`: the installed version, and only when
/// the formula really comes from the stack tap.
///
/// NOT `--installed` (aegis-fv4, measured on the Mac 2026-10-16): with that
/// flag brew ignores the formula argument and lists EVERY installed formula, so
/// the first entry was an unrelated `homebrew/core` package and no stack member
/// could ever produce a receipt ("brew receipt after install" on every run).
fn brew_info(brew: &std::ffi::OsStr, formula: &str) -> Option<String> {
    let out = Command::new(brew)
        .args(["info", "--json=v2", &format!("{STACK_TAP}/{formula}")])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    installed_version(&out.stdout, formula)
}

/// The installed version of exactly `<STACK_TAP>/<formula>` in brew's v2 JSON.
/// Selects the entry by full name rather than position, so extra or reordered
/// entries can never stand in for the member.
fn installed_version(json: &[u8], formula: &str) -> Option<String> {
    let info: serde_json::Value = serde_json::from_slice(json).ok()?;
    let wanted = format!("{STACK_TAP}/{formula}");
    let entry =
        info.get("formulae")?.as_array()?.iter().find(|entry| {
            entry.get("full_name").and_then(|v| v.as_str()) == Some(wanted.as_str())
        })?;
    if entry.get("tap")?.as_str()? != STACK_TAP {
        return None;
    }
    let version = entry
        .get("installed")?
        .as_array()?
        .first()?
        .get("version")?
        .as_str()?;
    // Homebrew marks rebuilds as `<version>_<n>`; the pin names the upstream.
    Some(version.split('_').next()?.to_owned())
}

/// The brew receipt for a member tool installed from the stack tap.
#[must_use]
pub fn brew_receipt(tool: ToolName) -> Option<Receipt> {
    let formula = brew_formula(tool)?;
    let version = brew_info(&brew_program()?, formula)?;
    Some(Receipt {
        manager: "brew",
        package: format!("{STACK_TAP}/{formula}"),
        version,
    })
}

/// The tap's current stable version for a member, when brew can read it.
#[must_use]
pub fn brew_available(tool: ToolName) -> Option<String> {
    let formula = brew_formula(tool)?;
    let out = Command::new(brew_program()?)
        .args(["info", "--json=v2", &format!("{STACK_TAP}/{formula}")])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let info: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let entry = info.get("formulae")?.as_array()?.first()?;
    if entry.get("tap")?.as_str()? != STACK_TAP {
        return None;
    }
    Some(entry.get("versions")?.get("stable")?.as_str()?.to_owned())
}

/// Install or upgrade a member from the stack tap. Brew runs as the user (no
/// root), so caboodle drives it directly (aegis-6f1 ruling).
pub fn brew_converge(tool: ToolName, upgrade: bool) -> anyhow::Result<()> {
    let formula = brew_formula(tool).ok_or_else(|| anyhow::anyhow!("no stack formula"))?;
    let brew = brew_program().ok_or_else(|| anyhow::anyhow!("brew is not available"))?;
    let verb = if upgrade { "upgrade" } else { "install" };
    let status = Command::new(&brew)
        .args([verb, &format!("{STACK_TAP}/{formula}")])
        .status()?;
    if !status.success() {
        anyhow::bail!("brew {verb} {STACK_TAP}/{formula} failed ({status})");
    }
    Ok(())
}

/// How a package-owned tool stands against its reviewed pin.
#[derive(Debug, PartialEq, Eq)]
pub enum Standing {
    Current,
    Ahead,
    Behind,
    /// One of the versions has no comparable semantic version.
    Unknown,
}

#[must_use]
pub fn standing(receipt_version: &str, desired: &str) -> Standing {
    let installed = format!("pkg {receipt_version}");
    match (
        crate::release_update::behind_reviewed(&installed, desired),
        crate::release_update::behind_reviewed(desired, &installed),
    ) {
        (Some(true), _) => Standing::Behind,
        (Some(false), Some(true)) => Standing::Ahead,
        (Some(false), Some(false)) => Standing::Current,
        _ => Standing::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::installed_version;

    fn entry(full_name: &str, tap: &str, installed: &[&str]) -> serde_json::Value {
        serde_json::json!({
            "full_name": full_name,
            "tap": tap,
            "installed": installed.iter().map(|v| serde_json::json!({"version": v})).collect::<Vec<_>>(),
        })
    }

    fn info(entries: Vec<serde_json::Value>) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"formulae": entries, "casks": []})).unwrap()
    }

    #[test]
    fn receipt_reads_the_named_member_not_the_first_installed_formula() {
        // aegis-fv4: `--installed` made brew list every formula; abseil came first.
        let listing = info(vec![
            entry("abseil", "homebrew/core", &["20250814.1"]),
            entry("scbrown/tap/quipu", "scbrown/tap", &["0.14.1"]),
        ]);
        assert_eq!(
            installed_version(&listing, "quipu").as_deref(),
            Some("0.14.1")
        );
        assert_eq!(installed_version(&listing, "bobbin"), None);
    }

    #[test]
    fn receipt_refuses_uninstalled_foreign_tap_and_malformed_answers() {
        assert_eq!(
            installed_version(
                &info(vec![entry("scbrown/tap/bobbin", "scbrown/tap", &[])]),
                "bobbin"
            ),
            None
        );
        assert_eq!(
            installed_version(
                &info(vec![entry("scbrown/tap/bobbin", "other/tap", &["1.0.0"])]),
                "bobbin"
            ),
            None
        );
        assert_eq!(installed_version(b"not json", "bobbin"), None);
        assert_eq!(
            installed_version(
                &info(vec![entry(
                    "scbrown/tap/yupana",
                    "scbrown/tap",
                    &["0.13.0_1"]
                )]),
                "yupana"
            )
            .as_deref(),
            Some("0.13.0")
        );
    }

    use super::*;

    #[test]
    fn debian_revision_and_epoch_are_not_part_of_the_pinned_version() {
        assert_eq!(upstream_version("0.13.0"), "0.13.0");
        assert_eq!(upstream_version("0.13.0-2"), "0.13.0");
        assert_eq!(upstream_version("1:0.13.0-2"), "0.13.0");
        assert_eq!(upstream_version("0.12.1-3"), "0.12.1");
    }

    #[test]
    fn caboodle_self_and_untapped_tools_have_no_stack_formula() {
        assert_eq!(brew_formula(ToolName::Bobbin), Some("bobbin"));
        assert_eq!(brew_formula(ToolName::DesirePath), Some("desire-path"));
        assert_eq!(brew_formula(ToolName::Member("seeds")), Some("seeds"));
        assert_eq!(brew_formula(ToolName::Member("chaski")), Some("chaski"));
        assert_eq!(brew_formula(ToolName::Camayoc), None);
        assert_eq!(brew_formula(ToolName::Member("shuttle")), None);
    }

    #[test]
    fn package_standing_never_reads_ahead_as_an_update() {
        assert_eq!(standing("0.13.0", "quipu 0.13.0"), Standing::Current);
        assert_eq!(standing("0.12.1", "quipu 0.13.0"), Standing::Behind);
        assert_eq!(standing("0.28.0", "bobbin 0.27.3"), Standing::Ahead);
        assert_eq!(standing("dev", "bobbin 0.27.3"), Standing::Unknown);
        assert_eq!(standing("0.7.1", "dp v0.3.2 (1f457ec)"), Standing::Ahead);
    }
}
