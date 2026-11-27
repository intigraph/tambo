//! Reviewed release pins for the five built-in tools (aegis-mecge2.10).
//!
//! The pins used to be Rust constants in `adapter.rs`, which no job could move:
//! the unattended bump job only rewrites `members/*.toml`, and a member file
//! may not shadow a built-in. They are now data in `builtins/pins.toml`,
//! embedded at build time and rewritten only by `tambo bump-builtin`.

use std::{collections::BTreeMap, fmt, ops::Deref, sync::OnceLock};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::model::ToolName;

/// The reviewed pin file, as shipped.
pub const SOURCE: &str = include_str!("../builtins/pins.toml");

/// Where a host-target executable is found and checked before a pin moves.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    /// Asset template (`{version}`, `{tag}`, `{target}`).
    pub asset: String,
    /// Its sums file; `{asset}` names the asset's own sibling.
    pub sums_asset: String,
}

/// One built-in tool's reviewed release.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BuiltinPin {
    pub repo: String,
    pub tag: String,
    pub version: String,
    /// The release tag's commit, for tools built or installed from source.
    #[serde(default)]
    pub revision: Option<String>,
    /// Digest-pinned asset template and the sums file that covers it.
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub sums_asset: Option<String>,
    #[serde(default)]
    pub sha256: BTreeMap<String, String>,
    /// Executable `bump-builtin` runs before recording a pin.
    #[serde(default)]
    pub program: Option<String>,
    /// Probe asset when the program is not in a digest-pinned asset.
    #[serde(default)]
    pub probe: Option<Probe>,
}

impl BuiltinPin {
    /// The release tag this pin names.
    #[must_use]
    pub fn tag_name(&self) -> String {
        self.tag.replace("{version}", &self.version)
    }

    /// `template` with `{version}`, `{tag}` and `{target}` filled in.
    #[must_use]
    pub fn fill(&self, template: &str, target: &str) -> String {
        template
            .replace("{tag}", &self.tag_name())
            .replace("{version}", &self.version)
            .replace("{target}", target)
    }

    fn validate(&self, name: &str) -> Result<()> {
        let safe = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '{' | '}'))
        };
        let hex = |s: &str, len: usize| s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit());
        let (owner, repo) = self
            .repo
            .split_once('/')
            .with_context(|| format!("{name}: repo must be owner/name"))?;
        if !safe(owner) || !safe(repo) || repo.contains('/') {
            bail!("{name}: unsafe repo {:?}", self.repo);
        }
        if !self.tag.contains("{version}") || !safe(&self.tag) {
            bail!(
                "{name}: tag template {:?} must contain {{version}}",
                self.tag
            );
        }
        if !self.version.starts_with(|c: char| c.is_ascii_digit()) {
            bail!("{name}: version {:?} must start with a digit", self.version);
        }
        crate::release_update::stable_version(&self.version)
            .with_context(|| format!("{name}: version {:?}", self.version))?;
        if let Some(revision) = &self.revision {
            if !hex(revision, 40) {
                bail!("{name}: revision must be a 40-character commit id");
            }
        }
        if self.asset.is_some() != self.sums_asset.is_some()
            || self.asset.is_some() == self.sha256.is_empty()
        {
            bail!("{name}: asset, sums_asset and [sha256] go together");
        }
        for (target, digest) in &self.sha256 {
            if !safe(target) || !hex(digest, 64) {
                bail!("{name}: bad sha256 entry {target}");
            }
        }
        if self.sha256.contains_key("any") && self.sha256.len() != 1 {
            bail!("{name}: `any` must be the only [sha256] key");
        }
        if self.probe.is_some() && self.program.is_none() {
            bail!("{name}: probe needs a program");
        }
        if self.program.is_some() && self.probe.is_none() && self.sha256.contains_key("any") {
            bail!("{name}: a program needs a per-target asset or a probe");
        }
        Ok(())
    }
}

/// Parse and validate a pin file: exactly the five built-ins, nothing else.
pub fn parse(text: &str) -> Result<BTreeMap<String, BuiltinPin>> {
    let pins: BTreeMap<String, BuiltinPin> =
        toml::from_str(text).context("parse builtins/pins.toml")?;
    let mut names: Vec<_> = ToolName::BUILTINS.iter().map(|t| t.as_str()).collect();
    names.sort_unstable();
    if pins.keys().map(String::as_str).collect::<Vec<_>>() != names {
        bail!("builtins/pins.toml must pin exactly {}", names.join(", "));
    }
    for (name, pin) in &pins {
        pin.validate(name)?;
    }
    Ok(pins)
}

fn all() -> &'static BTreeMap<String, BuiltinPin> {
    static PINS: OnceLock<BTreeMap<String, BuiltinPin>> = OnceLock::new();
    // The shipped file is validated by `the_shipped_pins_parse_and_validate`.
    PINS.get_or_init(|| parse(SOURCE).expect("embedded builtins/pins.toml is valid"))
}

/// The reviewed pin of `tool`.
#[must_use]
pub fn pin(tool: &str) -> &'static BuiltinPin {
    all()
        .get(tool)
        .unwrap_or_else(|| panic!("{tool} is not a built-in"))
}

/// A pinned value, usable where the old `const &str` was (`{PIN}` in
/// `format!`, `&PIN[..7]`, comparisons).
#[derive(Clone, Copy)]
pub struct Pin(pub fn() -> &'static str);

impl Deref for Pin {
    type Target = str;
    fn deref(&self) -> &str {
        (self.0)()
    }
}

impl fmt::Display for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}

impl PartialEq<Pin> for str {
    fn eq(&self, other: &Pin) -> bool {
        self == &**other
    }
}

impl PartialEq<Pin> for &str {
    fn eq(&self, other: &Pin) -> bool {
        *self == &**other
    }
}

impl PartialEq<Pin> for String {
    fn eq(&self, other: &Pin) -> bool {
        self.as_str() == &**other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_pins_parse_and_validate() {
        let pins = parse(SOURCE).unwrap();
        assert_eq!(pins.len(), ToolName::BUILTINS.len());
    }

    #[test]
    fn a_missing_extra_or_malformed_pin_is_refused() {
        let missing = SOURCE.replace("[bobbin]", "[not-bobbin]");
        assert!(parse(&missing).is_err());
        // Derived from the shipped pins, so a bump never breaks this test.
        let dp = pin("desire-path");
        let revision = dp.revision.as_deref().unwrap();
        let short = SOURCE.replace(revision, &revision[..8]);
        assert!(parse(&short).is_err());
        let prefixed = SOURCE.replace(
            &format!("version = \"{}\"", dp.version),
            &format!("version = \"v{}\"", dp.version),
        );
        assert!(parse(&prefixed).is_err());
        let digest_value = &pin("camayoc").sha256["any"];
        let digest = SOURCE.replace(digest_value.as_str(), &digest_value[..8]);
        assert!(parse(&digest).is_err());
        let unknown = SOURCE.replace("[quipu]\n", "[quipu]\nsurprise = 1\n");
        assert!(parse(&unknown).is_err());
    }

    #[test]
    fn templates_fill_from_the_pin() {
        let quipu = pin("quipu");
        assert_eq!(quipu.tag_name(), format!("quipu-ai-v{}", quipu.version));
        let probe = quipu.probe.as_ref().unwrap();
        assert_eq!(
            quipu.fill(&probe.asset, "aarch64-apple-darwin"),
            format!(
                "quipu-quipu-ai-v{}-aarch64-apple-darwin.tar.gz",
                quipu.version
            )
        );
    }

    #[test]
    fn a_pin_reads_like_the_constant_it_replaced() {
        const V: Pin = Pin(|| "1.2.3");
        assert_eq!(format!("x {V}"), "x 1.2.3");
        assert!("1.2.3" == V);
        let owned = format!("{}.{}.{}", 1, 2, 3);
        assert!(owned == V);
        assert_eq!(&V[..1], "1");
    }
}
