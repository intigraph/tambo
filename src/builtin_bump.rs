//! Bump a built-in tool's reviewed pin from its published release
//! (aegis-mecge2.10).
//!
//! The built-in counterpart of `member_bump`: the unattended bump job runs it
//! for quipu, bobbin, yupana, desire-path and camayoc, whose pins live in
//! `builtins/pins.toml`. It reads the release, refuses a downgrade, and:
//!
//! - re-derives every recorded digest by downloading the asset and requiring
//!   its bytes to hash to the release's published sums line;
//! - resolves `revision` (for tools built or installed from source) from the
//!   release tag's commit;
//! - for a tool with a `program`, unpacks the host target's verified asset and
//!   requires the program to report the new version, through
//!   `--probe-wrapper` when given, before anything is recorded.
//!
//! It rewrites only that tool's `version`, `revision` and `[sha256]` values
//! (comments and layout kept) for a human to review and merge. It never
//! installs anything.

use std::{fs, path::Path};

use anyhow::{bail, Context, Result};

use crate::adapter::download_https;
use crate::builtin_pins::{parse, BuiltinPin};
use crate::member_bump::{published_release, version_from_tag, Release};
use crate::release_update::{checksum, hash, stable_version};

/// What a bump did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Already pinned to the published release; nothing written.
    Current { version: String },
    /// The pin file now pins `to`; review and merge it.
    Bumped {
        from: String,
        to: String,
        revision: Option<String>,
        digests: Vec<(String, String)>,
    },
}

/// The host's release target for `tool`, in that tool's own target naming.
fn host_target(tool: &str) -> Result<&'static str> {
    match tool {
        "quipu" => crate::adapter::quipu_release_target(),
        "bobbin" => crate::adapter::bobbin_release_target(),
        "yupana" => {
            Ok(crate::adapter::yupana_release_for(std::env::consts::ARCH, std::env::consts::OS)?.0)
        }
        _ => bail!("{tool} has no executable release to prove"),
    }
}

fn safe_ref(tag: &str) -> Result<()> {
    if tag.is_empty()
        || !tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        bail!("unsafe release tag {tag:?}");
    }
    Ok(())
}

/// The commit the release tag points at.
fn tag_commit(repo: &str, tag: &str) -> Result<String> {
    safe_ref(tag)?;
    let raw = crate::release_update::release_metadata(&format!(
        "https://api.github.com/repos/{repo}/commits/{tag}"
    ))?;
    let value: serde_json::Value = serde_json::from_str(&raw).context("parse tag commit")?;
    let sha = value
        .get("sha")
        .and_then(serde_json::Value::as_str)
        .context("tag commit has no sha")?;
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("tag {tag} resolves to a malformed commit id");
    }
    Ok(sha.to_owned())
}

/// Download `asset` and require it to hash to its line in `sums_asset`.
fn verified_asset(
    tool: &str,
    release: &Release,
    base: &str,
    asset: &str,
    sums_asset: &str,
    dir: &Path,
) -> Result<(std::path::PathBuf, String)> {
    for name in [asset, sums_asset] {
        if !release.has(name) {
            bail!(
                "{tool}: release {} lacks a unique asset {name}",
                release.tag_name
            );
        }
    }
    let sums_path = dir.join(format!("{asset}.sums"));
    download_https(&format!("{base}/{sums_asset}"), &sums_path)?;
    let sums = fs::read_to_string(&sums_path).context("read published sums")?;
    let published = checksum(&sums, asset).with_context(|| format!("{sums_asset} for {asset}"))?;
    let local = dir.join(asset);
    download_https(&format!("{base}/{asset}"), &local)?;
    let actual = hash(&local)?;
    if actual != published {
        bail!("{tool}: {asset} hashes to {actual} but {sums_asset} publishes {published}; refusing to record either");
    }
    Ok((local, published))
}

/// Unpack `archive` and require `program` to report `version`.
fn prove(
    tool: &str,
    program: &str,
    archive: &Path,
    version: &str,
    wrapper: Option<&Path>,
) -> Result<()> {
    let scratch = tempfile::tempdir().context("create bump proof directory")?;
    let unpack = scratch.path().join("unpack");
    fs::create_dir_all(&unpack)?;
    crate::adapter::checked(
        "tar",
        [
            std::ffi::OsStr::new("-xzf"),
            archive.as_os_str(),
            std::ffi::OsStr::new("-C"),
            unpack.as_os_str(),
        ],
        None,
    )?;
    let path = crate::adapter::find_program(&unpack, program)
        .with_context(|| format!("{tool}: release archive has no {program}"))?;
    let argv = [path.to_string_lossy().into_owned(), "--version".to_owned()];
    let out = crate::adapter::hermetic_via(scratch.path(), wrapper, &argv, None)
        .with_context(|| format!("{tool}: run the release's {program} --version"))?;
    let reported = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        bail!(
            "{tool}: {program} --version failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    if !reported
        .split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .any(|word| word.trim_start_matches('v') == version)
    {
        bail!(
            "{tool}: the release's {program} reports {:?}, not {version}",
            reported.trim()
        );
    }
    Ok(())
}

/// Bump `tool` in the pin file at `path` to the latest published release, or
/// to `tag`.
pub fn bump(
    path: &Path,
    tool: &str,
    tag: Option<&str>,
    probe_wrapper: Option<&Path>,
) -> Result<Outcome> {
    let original = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let pins = parse(&original)?;
    let current = pins
        .get(tool)
        .with_context(|| format!("{tool} is not a built-in tool"))?;
    if let Some(tag) = tag {
        safe_ref(tag)?;
    }
    let release = published_release(&current.repo, tag)?;
    let version = version_from_tag(&current.tag, &release.tag_name)?;
    let (was, now) = (stable_version(&current.version)?, stable_version(&version)?);
    if now == was {
        return Ok(Outcome::Current { version });
    }
    if now < was {
        bail!(
            "{tool}: published {} is older than the pinned {}; refusing to downgrade the reviewed pin",
            release.tag_name,
            current.version
        );
    }
    let mut next: BuiltinPin = current.clone();
    next.version.clone_from(&version);
    let base = format!(
        "https://github.com/{}/releases/download/{}",
        next.repo, release.tag_name
    );
    let scratch = tempfile::tempdir().context("create bump download directory")?;

    let revision = match &current.revision {
        Some(_) => Some(tag_commit(&next.repo, &release.tag_name)?),
        None => None,
    };
    next.revision.clone_from(&revision);

    let host = if next.program.is_some() {
        Some(
            host_target(tool)
                .context("bump-builtin must run on a pinned target so it can prove the release")?,
        )
    } else {
        None
    };
    let mut proved = false;
    let mut digests = Vec::new();
    if let (Some(asset), Some(sums_asset)) = (&next.asset, &next.sums_asset) {
        // The reviewed target set is the one already pinned: a bump never adds
        // or silently drops a platform.
        for target in current.sha256.keys() {
            let name = next.fill(asset, target);
            let (local, published) =
                verified_asset(tool, &release, &base, &name, sums_asset, scratch.path())?;
            if Some(target.as_str()) == host {
                let program = next.program.as_deref().context("program")?;
                prove(tool, program, &local, &version, probe_wrapper)?;
                proved = true;
            }
            digests.push((target.clone(), published));
        }
    }
    if let (Some(probe), Some(host)) = (&next.probe, host) {
        let asset = next.fill(&probe.asset, host);
        let sums_asset = probe.sums_asset.replace("{asset}", &asset);
        let (local, _) =
            verified_asset(tool, &release, &base, &asset, &sums_asset, scratch.path())?;
        let program = next.program.as_deref().context("program")?;
        prove(tool, program, &local, &version, probe_wrapper)?;
        proved = true;
    }
    if next.program.is_some() && !proved {
        bail!(
            "{tool}: the {} release was not proved; refusing to record",
            host.unwrap_or("host")
        );
    }

    let mut document: toml_edit::DocumentMut = original
        .parse()
        .context("parse builtins/pins.toml for editing")?;
    document[tool]["version"] = toml_edit::value(version.clone());
    if let Some(revision) = &revision {
        document[tool]["revision"] = toml_edit::value(revision.clone());
    }
    for (target, digest) in &digests {
        document[tool]["sha256"][target.as_str()] = toml_edit::value(digest.clone());
    }
    let written = document.to_string();
    // Read back what will be written before replacing the reviewed file: only
    // this tool changed, and exactly as bumped.
    for (target, digest) in &digests {
        next.sha256.insert(target.clone(), digest.clone());
    }
    let check = parse(&written)?;
    let unchanged = pins
        .iter()
        .filter(|(name, _)| name.as_str() != tool)
        .all(|(name, pin)| check.get(name) == Some(pin));
    if check.get(tool) != Some(&next) || !unchanged {
        bail!("{tool}: rewritten pin file does not read back as the bump");
    }
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staged = tempfile::NamedTempFile::new_in(dir)?;
    fs::write(staged.path(), &written)?;
    fs::set_permissions(staged.path(), fs::metadata(path)?.permissions())?;
    staged
        .persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(Outcome::Bumped {
        from: current.version.clone(),
        to: version,
        revision,
        digests,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_tool_or_unsafe_tag_is_refused_before_any_network() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pins.toml");
        fs::write(&path, crate::builtin_pins::SOURCE).unwrap();
        let unknown = bump(&path, "seeds", None, None).unwrap_err().to_string();
        assert!(unknown.contains("not a built-in"), "{unknown}");
        let unsafe_tag = bump(&path, "bobbin", Some("v1/../x"), None)
            .unwrap_err()
            .to_string();
        assert!(unsafe_tag.contains("unsafe release tag"), "{unsafe_tag}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            crate::builtin_pins::SOURCE
        );
    }

    #[test]
    fn a_program_reports_the_version_with_or_without_a_v() {
        let dir = tempfile::tempdir().unwrap();
        let tree = dir.path().join("tree/demo-1.2.3");
        fs::create_dir_all(&tree).unwrap();
        let program = tree.join("demo");
        fs::write(&program, "#!/bin/sh\necho 'demo v1.2.3 (abcdef0)'\n").unwrap();
        fs::set_permissions(
            &program,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
        let archive = dir.path().join("demo.tar.gz");
        crate::adapter::checked(
            "tar",
            [
                std::ffi::OsStr::new("-czf"),
                archive.as_os_str(),
                std::ffi::OsStr::new("-C"),
                dir.path().join("tree").as_os_str(),
                std::ffi::OsStr::new("demo-1.2.3"),
            ],
            None,
        )
        .unwrap();
        prove("demo", "demo", &archive, "1.2.3", None).unwrap();
        let wrong = prove("demo", "demo", &archive, "1.2.4", None)
            .unwrap_err()
            .to_string();
        assert!(wrong.contains("not 1.2.4"), "{wrong}");
    }
}
