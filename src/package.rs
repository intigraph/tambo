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
    use super::*;

    #[test]
    fn debian_revision_and_epoch_are_not_part_of_the_pinned_version() {
        assert_eq!(upstream_version("0.13.0"), "0.13.0");
        assert_eq!(upstream_version("0.13.0-2"), "0.13.0");
        assert_eq!(upstream_version("1:0.13.0-2"), "0.13.0");
        assert_eq!(upstream_version("0.12.1-3"), "0.12.1");
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
