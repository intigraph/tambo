use super::{
    native::{self, Native},
    read, write_json,
};
use crate::skill_profile::{self, CatalogueSnapshot, Harness, Pin, Release, SkillProfile};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    skill: String,
    name: String,
    description: String,
    repository: String,
    source_path: String,
    version: String,
    render_spec: String,
    public: bool,
    harnesses: Vec<Harness>,
}
impl Metadata {
    fn release(&self, share_id: String) -> Release {
        Release {
            pin: Pin {
                skill: self.skill.clone(),
                version: self.version.clone(),
                share_id,
            },
            name: self.name.clone(),
            render_spec: self.render_spec.clone(),
            public: self.public,
            harnesses: self.harnesses.clone(),
        }
    }
    fn validate(&self) -> Result<()> {
        // Reuse the selector's destination/pin/harness checks before native I/O.
        let release = self.release(format!("sha256:{}", "0".repeat(64)));
        let profile = SkillProfile {
            id: self.skill.clone(),
            public_only: self.public,
            harnesses: self.harnesses.clone(),
            pins: vec![release.pin.clone()],
        };
        skill_profile::select(
            &profile,
            &CatalogueSnapshot {
                transaction: 1,
                complete: true,
                releases: vec![release],
            },
        )?;
        for iri in [&self.skill, &self.repository, &self.render_spec] {
            skill_profile::query::plan(iri, "urn:fixture:validation", 1)?;
        }
        if self.description.is_empty()
            || self.description.len() > 4096
            || self.source_path.is_empty()
            || self.source_path.len() > 1024
        {
            bail!("skill description/source path exceeds catalogue contract");
        }
        Ok(())
    }
    fn turtle(&self, share: &str) -> Result<String> {
        let release = format!(
            "{}:release:{}",
            self.skill,
            share
                .strip_prefix("sha256:")
                .context("native share has no algorithm tag")?
        );
        let mut out = format!("<{release}> a <urn:quipu:skills:Release> .\n");
        for (key, value, iri) in [
            ("skill", self.skill.as_str(), true),
            ("name", &self.name, false),
            ("description", &self.description, false),
            ("repository", &self.repository, true),
            ("sourcePath", &self.source_path, false),
            ("version", &self.version, false),
            ("shareId", share, false),
            ("renderSpec", &self.render_spec, true),
            (
                "scope",
                if self.public { "public" } else { "internal" },
                false,
            ),
        ] {
            let value = if iri {
                format!("<{value}>")
            } else {
                serde_json::to_string(value)?
            };
            out.push_str(&format!("<{release}> <urn:quipu:skills:{key}> {value} .\n"));
        }
        for harness in &self.harnesses {
            out.push_str(&format!(
                "<{release}> <urn:quipu:skills:harness> {} .\n",
                serde_json::to_string(harness)?
            ));
        }
        Ok(out)
    }
}

pub(super) fn publish(
    source: &Path,
    metadata: &Path,
    shapes: &Path,
    policy: Option<(&Path, &Path)>,
    output: &Path,
    program: &Path,
) -> Result<Value> {
    let meta: Metadata = read(metadata)?;
    meta.validate()?;
    if fs::symlink_metadata(output).is_ok() {
        bail!("skill release output already exists");
    }
    let spec = native::compile(source)?;
    let stage = tempfile::tempdir()?;
    let frozen_shapes = stage.path().join("render-shapes.ttl");
    fs::write(
        &frozen_shapes,
        native::regular_bytes(shapes, 4 * 1024 * 1024)?,
    )?;
    let native = Native::new(program, stage.path().join("producer.db"));
    native.spec(
        &meta.render_spec,
        &spec,
        &frozen_shapes,
        &stage.path().join("source.ttl"),
    )?;
    if let Some((data, shape)) = policy {
        let frozen_data = stage.path().join("identifier-policy.ttl");
        let frozen_shape = stage.path().join("identifier-shapes.ttl");
        fs::write(&frozen_data, native::regular_bytes(data, 4 * 1024 * 1024)?)?;
        fs::write(
            &frozen_shape,
            native::regular_bytes(shape, 4 * 1024 * 1024)?,
        )?;
        native.call(&[
            "shapes".as_ref(),
            "load".as_ref(),
            "identifier-policy".as_ref(),
            frozen_shape.as_os_str(),
        ])?;
        native.call(&[
            "knot".as_ref(),
            frozen_data.as_os_str(),
            "--graph".as_ref(),
            "urn:tambo:identifier-policy".as_ref(),
        ])?;
    }
    let share = stage.path().join("pendant");
    native.call(&[
        "share".as_ref(),
        "--output".as_ref(),
        share.as_os_str(),
        "--no-shapes".as_ref(),
        "--no-queries".as_ref(),
        "--destination".as_ref(),
        if meta.public {
            "outward".as_ref()
        } else {
            "internal".as_ref()
        },
    ])?;
    let manifest: Value = read(&share.join("manifest.json"))?;
    let id = manifest["share_id"]
        .as_str()
        .context("native share lacks share_id")?;
    let release = meta.release(id.into());
    let catalogue_turtle = meta.turtle(id)?;
    if meta.public {
        // The catalogue holds the already-computed share ID. Check its outward
        // metadata too; a share cannot embed its own hash in the hashed bytes.
        let file = stage.path().join("catalogue.ttl");
        let checked = stage.path().join("catalogue-check");
        fs::write(&file, &catalogue_turtle)?;
        native.call(&["knot".as_ref(), file.as_os_str()])?;
        native.call(&[
            "share".as_ref(),
            "--output".as_ref(),
            checked.as_os_str(),
            "--no-shapes".as_ref(),
            "--no-queries".as_ref(),
            "--destination".as_ref(),
            "outward".as_ref(),
        ])?;
    }
    let profile = SkillProfile {
        id: meta.skill.clone(),
        public_only: meta.public,
        harnesses: meta.harnesses.clone(),
        pins: vec![release.pin.clone()],
    };
    skill_profile::select(
        &profile,
        &CatalogueSnapshot {
            transaction: 1,
            complete: true,
            releases: vec![release.clone()],
        },
    )?;
    let verifier = Native::new(program, stage.path().join("verify.db"));
    let verified = verifier.json(&["import".as_ref(), share.as_os_str()])?;
    if verified["share_id"] != id
        || !matches!(verified["outcome"].as_str(), Some("staged" | "quarantined"))
    {
        bail!("native import did not verify the published share");
    }
    let archive = stage.path().join("bundle.pendant.tar.gz");
    let encoder =
        flate2::write::GzEncoder::new(fs::File::create(&archive)?, flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);
    tar.append_dir_all("pendant", &share)?;
    tar.into_inner()?.finish()?.sync_all()?;
    // Reserve a NEW release directory. Never replace an existing release.
    fs::create_dir(output)?;
    fs::copy(&archive, output.join("bundle.pendant.tar.gz"))?;
    fs::write(output.join("catalogue.ttl"), catalogue_turtle)?;
    write_json(&output.join("metadata.json"), &meta)?;
    // Publish the final marker atomically: failed I/O cannot leave a partial
    // release.json masquerading as a complete release. Earlier files remain
    // inspectable on failure; retries never overwrite the reserved directory.
    let marker = tempfile::NamedTempFile::new_in(output)?;
    serde_json::to_writer_pretty(marker.as_file(), &release)?;
    marker.as_file().sync_all()?;
    marker.persist_noclobber(output.join("release.json"))?;
    Ok(
        json!({"outcome":"published", "release":release, "verification":verified,
        "catalogue_registered":false, "output":output}),
    )
}
