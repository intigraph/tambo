//! Pure pinned skill selection. This does not query, import, or install anything.
//!
//! A future graph adapter must establish the profile's governed type, explicit
//! dataset and transaction before constructing these inputs. A complete response
//! is query coverage, not proof that a whole graph has been inventoried. Pendant
//! verification and render ownership remain the native tools' responsibility.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The two supported materialization consumers, never arbitrary directory names.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Harness {
    Claude,
    Codex,
}

/// One exact native pendant identity selected by a profile.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub skill: String,
    pub share_id: String,
    pub version: String,
}

/// A profile selects immutable releases; it never asks for "latest".
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillProfile {
    pub id: String,
    pub public_only: bool,
    pub harnesses: Vec<Harness>,
    pub pins: Vec<Pin>,
}

/// Metadata for a native pendant. File contents do not belong in this model.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub pin: Pin,
    pub name: String,
    pub render_spec: String,
    pub public: bool,
    pub harnesses: Vec<Harness>,
}

/// A bounded, transaction-pinned result supplied by the graph adapter.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogueSnapshot {
    pub transaction: u64,
    pub complete: bool,
    pub releases: Vec<Release>,
}

/// A deterministic selection, not evidence of installation or trust admission.
#[derive(Clone, Debug, Serialize)]
pub struct Selection {
    pub profile: String,
    pub transaction: u64,
    pub harnesses: Vec<Harness>,
    pub releases: Vec<Release>,
}

fn text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 2_048
        && !value.chars().any(|c| c.is_control() || c.is_whitespace())
}

fn valid_pin(pin: &Pin) -> bool {
    text(&pin.skill)
        && text(&pin.version)
        && pin.share_id.strip_prefix("sha256:").is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

fn harnesses(input: &[Harness]) -> Result<BTreeSet<Harness>> {
    let set: BTreeSet<_> = input.iter().copied().collect();
    if set.is_empty() || set.len() != input.len() {
        bail!("empty or duplicate harness selection");
    }
    Ok(set)
}

/// Resolve exact pins, refusing partial or ambiguous metadata before any I/O.
///
/// Empty profiles are deliberately unsupported until explicit retirement is
/// implemented. This prevents an absent catalogue from becoming a delete-all
/// instruction. Identifiers remain opaque here: the graph adapter must parse
/// them as RDF terms, and no identifier is interpreted as a filesystem path.
pub fn select(profile: &SkillProfile, snapshot: &CatalogueSnapshot) -> Result<Selection> {
    if !text(&profile.id) || !snapshot.complete || snapshot.transaction == 0 {
        bail!("profile or transaction-pinned catalogue is unavailable");
    }
    if profile.pins.is_empty() || profile.pins.len() > 128 || snapshot.releases.len() > 1_024 {
        bail!("skill selection exceeds its bounded contract");
    }
    let requested = harnesses(&profile.harnesses)?;
    let mut catalogue = BTreeMap::new();
    for release in &snapshot.releases {
        if !valid_pin(&release.pin)
            || !text(&release.render_spec)
            || release.name.is_empty()
            || release.name.len() > 64
            || !release
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            bail!("invalid skill release metadata");
        }
        harnesses(&release.harnesses)?;
        let key = (&release.pin.skill, &release.pin.share_id);
        if catalogue.insert(key, release).is_some() {
            bail!("ambiguous skill release metadata");
        }
    }
    let mut selected = BTreeMap::new();
    let mut names = BTreeSet::new();
    for pin in &profile.pins {
        if !valid_pin(pin) {
            bail!("invalid profile pin");
        }
        let release = catalogue
            .get(&(&pin.skill, &pin.share_id))
            .ok_or_else(|| anyhow::anyhow!("pinned skill release is unavailable"))?;
        if pin.version != release.pin.version
            || (profile.public_only && !release.public)
            || !requested.is_subset(&harnesses(&release.harnesses)?)
        {
            bail!("pinned release does not satisfy the profile");
        }
        if selected
            .insert(pin.skill.clone(), (**release).clone())
            .is_some()
            || !names.insert(release.name.clone())
        {
            bail!("duplicate skill identity or destination name");
        }
    }
    Ok(Selection {
        profile: profile.id.clone(),
        transaction: snapshot.transaction,
        harnesses: requested.into_iter().collect(),
        releases: selected.into_values().collect(),
    })
}

#[cfg(test)]
mod tests;
