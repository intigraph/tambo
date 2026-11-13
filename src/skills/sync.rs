use super::{
    native::{self, Native},
    read,
};
use crate::skill_profile::{self, CatalogueSnapshot, Harness, SkillProfile};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub(super) fn sync(
    profile_path: &Path,
    catalogue: &Path,
    bundles: &Path,
    authority: &Path,
    shapes: &Path,
    dest: &Path,
    program: &Path,
) -> Result<Value> {
    let profile: SkillProfile = read(profile_path)?;
    let catalogue: CatalogueSnapshot = read(catalogue)?;
    let selected = skill_profile::select(&profile, &catalogue)?;
    // File inputs bypass the graph decoder, so validate their RDF identifiers
    // before interpreting any selected render specification.
    for iri in std::iter::once(profile.id.as_str()).chain(
        selected
            .releases
            .iter()
            .flat_map(|r| [r.pin.skill.as_str(), r.render_spec.as_str()]),
    ) {
        skill_profile::query::plan(
            iri,
            "urn:tambo:local-skill-catalogue",
            catalogue.transaction,
        )?;
    }
    let paths: BTreeMap<String, PathBuf> = read(bundles)?;
    let stage = tempfile::tempdir()?;
    let frozen_shapes = stage.path().join("render-shapes.ttl");
    fs::write(
        &frozen_shapes,
        native::regular_bytes(shapes, 4 * 1024 * 1024)?,
    )?;
    let native = Native::new(program, stage.path().join("compose.db"));
    let repo = format!("skills/{}", hex(profile.id.as_bytes()));
    let graph = format!("urn:quipu:repo:{repo}:current");
    let mut budget = 256 * 1024 * 1024;
    let mut decoded_budget = 256 * 1024 * 1024;
    let auth = freeze(
        authority,
        &stage.path().join("authority.pendant.tar.gz"),
        &mut budget,
    )?;
    let authority_manifest = crate::pendant_projects::manifest(&auth, &mut decoded_budget)?;
    let mut inputs = vec![format!("authority={}", auth.display())];
    for (i, release) in selected.releases.iter().enumerate() {
        let source = paths
            .get(&release.pin.share_id)
            .context("selected native pendant unavailable locally")?;
        let target = stage.path().join(format!("skill-{i}.pendant.tar.gz"));
        freeze(source, &target, &mut budget)?;
        let manifest = crate::pendant_projects::manifest(&target, &mut decoded_budget)?;
        if manifest.share_id != release.pin.share_id {
            bail!("bundle identity differs from selected pin");
        }
        inputs.push(format!("skill{i}={}", target.display()));
    }
    let mut args: Vec<&std::ffi::OsStr> = vec![
        "compose".as_ref(),
        "--repo".as_ref(),
        repo.as_ref(),
        "--shapes-from".as_ref(),
        "authority".as_ref(),
        "--destination".as_ref(),
        if profile.public_only {
            "outward".as_ref()
        } else {
            "internal".as_ref()
        },
    ];
    args.extend(inputs.iter().map(std::ffi::OsStr::new));
    let composed = native.json(&args)?;
    if composed["repo"] != repo
        || composed["composition"]["outcome"] != "composed"
        || composed["current"]["dataset"] != graph
    {
        bail!("native composition did not admit expected CURRENT");
    }
    let mut includes = Vec::new();
    let mut encoded_budget = 16usize * 1024 * 1024;
    for (i, release) in selected.releases.iter().enumerate() {
        let rendered = stage.path().join(format!("rendered-{i}"));
        native.render(&release.render_spec, Some(&graph), &rendered)?;
        // Ownership metadata belongs to the native renderer, not the skill.
        // This is our private temporary render, never an installed directory.
        fs::remove_file(rendered.join(".quipu-render.json"))?;
        let mut spec = native::compile(&rendered)?;
        let content = fs::read_to_string(rendered.join("SKILL.md"))?;
        // Native render owns this 0700 temporary tree; compile rechecks no-follow,
        // path/mode/byte bounds before any aggregate destination is touched.
        let supports = spec["includes"]
            .as_array_mut()
            .context("compiled skill lacks includes")?;
        supports.push(
            json!({"path":"SKILL.md", "sha256":format!("sha256:{}", hex(content.as_bytes())),
            "base64":encode(content.as_bytes()), "mode":420}),
        );
        for harness in &selected.harnesses {
            let harness = match harness {
                Harness::Claude => "claude",
                Harness::Codex => "codex",
            };
            for support in supports.iter() {
                encoded_budget = encoded_budget
                    .checked_sub(
                        support["base64"]
                            .as_str()
                            .context("compiled include lacks bytes")?
                            .len(),
                    )
                    .context("whole skill profile exceeds encoded-byte budget")?;
                let mut item = support.clone();
                item["path"] = format!(
                    "{harness}/{}/{}",
                    release.name,
                    support["path"].as_str().context("invalid compiled path")?
                )
                .into();
                includes.push(item);
                if includes.len() > 512 {
                    bail!("whole skill profile exceeds file budget");
                }
            }
        }
    }
    // The renderer, not this adapter, owns atomic publication and retirement.
    // Its refusal on unknown/changed destination files preserves foreign entries.
    let receipt = json!({"version":1, "selection":selected,
        "authority_share_id":authority_manifest.share_id, "harness_registration":false});
    let render = Native::new(program, stage.path().join("profile.db"));
    let iri = format!("urn:tambo:skill-profile:{}", hex(profile.id.as_bytes()));
    render.spec(
        &iri,
        &native::single(
            &serde_json::to_string_pretty(&receipt)?,
            "profile.json",
            includes,
        ),
        &frozen_shapes,
        &stage.path().join("profile.ttl"),
    )?;
    let result = render.render(&iri, None, dest)?;
    Ok(
        json!({"outcome":"materialized", "profile":receipt, "render":result, "composition":composed,
        "destination":dest, "harness_registration":false, "catalogue_source":"caller-supplied files; not server authentication"}),
    )
}

fn hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn freeze(source: &Path, target: &Path, budget: &mut u64) -> Result<PathBuf> {
    let bytes = native::regular_bytes(source, *budget)?;
    *budget = budget
        .checked_sub(bytes.len() as u64)
        .context("skill archive budget exceeded")?;
    fs::write(target, bytes)?;
    Ok(target.into())
}
fn encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let x = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(TABLE[(x >> 18) as usize] as char);
        out.push(TABLE[((x >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((x >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(x & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
