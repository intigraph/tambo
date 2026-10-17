//! Project selection reads the native share scope; there is no repository manifest.
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{fs, io::Read, path::Path};

#[derive(Deserialize)]
pub struct Manifest {
    pub share_id: String,
    pub parent_share: Option<String>,
    pub scope: Scope,
}

#[derive(Deserialize)]
pub struct Scope {
    pub kind: String,
    pub value: Option<String>,
}

/// Read metadata without extracting any archive paths. Quipu subsequently verifies
/// the complete frozen archive before it can change CURRENT.
pub fn manifest(path: &Path, budget: &mut u64) -> Result<Manifest> {
    let compressed = fs::File::open(path)?;
    let decoder = flate2::read::GzDecoder::new(compressed);
    let mut limited = decoder.take(*budget);
    let result = (|| {
        let mut archive = tar::Archive::new(&mut limited);
        let mut found = None;
        for entry in archive.entries()? {
            let mut entry = entry?;
            let name = entry.path()?.into_owned();
            if name.file_name().is_some_and(|n| n == "manifest.json") {
                if found.is_some() || !entry.header().entry_type().is_file() {
                    bail!("project archive needs exactly one regular manifest.json");
                }
                if entry.size() > 1024 * 1024 {
                    bail!("project share manifest exceeds 1 MiB");
                }
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes)?;
                found = Some(serde_json::from_slice(&bytes)?);
            }
        }
        found.context("project archive has no native share manifest")
    })();
    let remaining = limited.limit();
    let exhausted = remaining == 0;
    *budget = remaining;
    if exhausted {
        bail!("project archives exceed the 256 MiB decompressed limit");
    }
    result
}

pub fn valid_prefix(prefix: &str) -> Result<()> {
    if !prefix.ends_with('/')
        || prefix.starts_with('/')
        || prefix.split('/').any(|p| p == "." || p == "..")
        || prefix
            .strip_suffix('/')
            .unwrap_or(prefix)
            .split('/')
            .any(str::is_empty)
        || prefix.chars().any(|c| c.is_control() || c == '\\')
        || prefix == "/"
    {
        bail!("project prefix must be a nonempty relative directory ending in /: {prefix:?}");
    }
    Ok(())
}

fn iri(value: &str) -> Result<()> {
    if !value.contains(':')
        || value
            .chars()
            .any(|c| c.is_whitespace() || "<>\"{}|^`\\".contains(c))
    {
        bail!("expected an absolute IRI: {value:?}");
    }
    Ok(())
}

/// Canonical native CONSTRUCT scope, also used when reading a derived share.
pub fn query(graph: &str, predicate: &str, prefix: &str) -> Result<String> {
    enriched_query(graph, predicate, prefix, &[])
}

pub fn enriched_query(
    graph: &str,
    predicate: &str,
    prefix: &str,
    owners: &[crate::pendant_ownership::OwnedPath],
) -> Result<String> {
    valid_prefix(prefix)?;
    iri(graph)?;
    iri(predicate)?;
    let mut template = "CONSTRUCT { ?s ?p ?o".to_owned();
    let mut optional = String::new();
    for (i, owner) in owners.iter().enumerate() {
        iri(&owner.predicate)?;
        iri(&owner.owner)?;
        valid_prefix(&format!("{}/", owner.path))?;
        if !owner.path.starts_with(prefix) {
            bail!("ownership path lies outside the project");
        }
        template.push_str(&format!(
            " . ?owned{i} <{}> <{}>",
            owner.predicate, owner.owner
        ));
        optional.push_str(&format!(
            " UNION {{ GRAPH <{graph}> {{ ?owned{i} <{predicate}> {} }} }}",
            serde_json::to_string(&owner.path)?
        ));
    }
    let core = format!(
        "GRAPH <{graph}> {{ ?s <{predicate}> ?path . ?s ?p ?o . FILTER(STRSTARTS(?path, {})) }}",
        serde_json::to_string(prefix)?
    );
    let body = if owners.is_empty() {
        core
    } else {
        format!("{{ {core} }}{optional}")
    };
    Ok(format!("{template} }} WHERE {{ {body} }}"))
}

fn literal(input: &str) -> Result<(String, &str)> {
    let mut stream = serde_json::Deserializer::from_str(input).into_iter::<String>();
    let value = stream.next().context("missing scope literal")??;
    let offset = stream.byte_offset();
    Ok((value, &input[offset..]))
}

pub fn prefix(scope: &Scope) -> Result<String> {
    if scope.kind != "construct" {
        bail!("derived project share needs a CONSTRUCT path-prefix scope");
    }
    let value = scope.value.as_deref().context("missing CONSTRUCT scope")?;
    let (mut template, rest) = value
        .strip_prefix("CONSTRUCT { ?s ?p ?o")
        .and_then(|v| v.split_once(" } WHERE { "))
        .context("unsupported project scope; use packs project-scope")?;
    let enriched = !template.is_empty();
    let rest = if enriched {
        rest.strip_prefix("{ GRAPH <")
    } else {
        rest.strip_prefix("GRAPH <")
    }
    .context("unsupported project graph pattern")?;
    let (graph, rest) = rest
        .split_once("> { ?s <")
        .context("unsupported project source graph")?;
    let (predicate, tail) = rest
        .split_once("> ?path . ?s ?p ?o . FILTER(STRSTARTS(?path, ")
        .context("unsupported project scope")?;
    let (prefix, tail) = literal(tail)?;
    let mut tail = tail
        .strip_prefix(")) }")
        .context("unsupported project scope suffix")?;
    if enriched {
        tail = tail
            .strip_prefix(" }")
            .context("unsupported project union")?;
    }
    let mut owners = Vec::new();
    while !template.is_empty() {
        let i = owners.len();
        let rest = template
            .strip_prefix(&format!(" . ?owned{i} <"))
            .context("unsupported ownership template")?;
        let (owned_by, rest) = rest
            .split_once("> <")
            .context("unsupported ownership predicate")?;
        let (owner, rest) = rest.split_once('>').context("unsupported ownership IRI")?;
        template = rest;
        tail = tail
            .strip_prefix(&format!(
                " UNION {{ GRAPH <{graph}> {{ ?owned{i} <{predicate}> "
            ))
            .context("unsupported ownership scope")?;
        let (path, rest) = literal(tail)?;
        tail = rest
            .strip_prefix(" } }")
            .context("unsupported ownership binding")?;
        owners.push(crate::pendant_ownership::OwnedPath {
            path,
            predicate: owned_by.into(),
            owner: owner.into(),
        });
        if owners.len() > 4096 {
            bail!("ownership scope exceeds 4096 assignments");
        }
    }
    if tail != " }" || enriched_query(graph, predicate, &prefix, &owners)? != value {
        bail!("noncanonical project scope");
    }
    Ok(prefix)
}

pub fn present(root: &Path, prefix: &str) -> Result<bool> {
    valid_prefix(prefix)?;
    let mut path = root.to_path_buf();
    for part in prefix.trim_end_matches('/').split('/') {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.into()),
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => (),
            Ok(_) => bail!(
                "project path must contain only regular directories: {}",
                path.display()
            ),
        }
    }
    Ok(true)
}

fn checked_out(root: &Path, prefix: &str) -> Result<bool> {
    if !present(root, prefix)? {
        return Ok(false);
    }
    let listed = crate::pendant_sync::git_command(root)
        .args(["--literal-pathspecs", "ls-files", "-v", "-z", "--", prefix])
        .output()?;
    if !listed.status.success() {
        bail!("cannot inspect project checkout membership");
    }
    for entry in String::from_utf8(listed.stdout)?
        .split('\0')
        .filter(|s| !s.is_empty())
    {
        let (tag, path) = entry
            .split_once(' ')
            .context("invalid Git checkout entry")?;
        if tag.len() != 1 || !path.starts_with(prefix) {
            bail!("Git returned an unexpected project path");
        }
        if matches!(tag, "S" | "s") {
            continue;
        }
        match fs::symlink_metadata(root.join(path)) {
            Ok(m) if m.is_file() || m.file_type().is_symlink() => return Ok(true),
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(false)
}

pub fn select(
    root: &Path,
    inputs: Vec<(String, std::path::PathBuf)>,
) -> Result<Vec<(String, std::path::PathBuf)>> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let mut budget = 256 * 1024 * 1024;
    let manifests = inputs
        .iter()
        .map(|(_, p)| manifest(p, &mut budget))
        .collect::<Result<Vec<_>>>()?;
    let ids: std::collections::BTreeSet<_> = manifests.iter().map(|m| &m.share_id).collect();
    if ids.len() != manifests.len() {
        bail!("duplicate share identity in monorepo archives");
    }
    // A parent may itself have a previous snapshot in its lineage. Find the
    // parent referenced by this archive set, rather than assuming a null parent.
    let parents: Vec<_> = manifests
        .iter()
        .filter(|m| {
            manifests
                .iter()
                .any(|child| child.parent_share.as_ref() == Some(&m.share_id))
        })
        .collect();
    if parents.len() != 1 {
        bail!("monorepo sync needs exactly one parent share and derived project shares");
    }
    let parent = &parents[0].share_id;
    let mut prefixes = std::collections::BTreeSet::new();
    let mut selected = Vec::new();
    for (input, manifest) in inputs.into_iter().zip(manifests.iter()) {
        if &manifest.share_id == parent {
            continue;
        }
        let id = manifest
            .parent_share
            .as_ref()
            .context("project share has no parent")?;
        if id != parent {
            bail!("derived project share names a different parent");
        }
        let prefix = prefix(&manifest.scope)?;
        if !prefixes.insert(prefix.clone()) {
            bail!("duplicate project prefix: {prefix}");
        }
        if checked_out(root, &prefix)? {
            selected.push(input);
        }
    }
    if prefixes.is_empty() {
        bail!("monorepo sync has no derived project shares");
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(
        root: &Path,
        name: &str,
        id: &str,
        parent: Option<&str>,
        scope: serde_json::Value,
    ) -> std::path::PathBuf {
        let path = root.join(format!("{name}.pendant.tar.gz"));
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            fs::File::create(&path).unwrap(),
            flate2::Compression::default(),
        ));
        let data = serde_json::to_vec(
            &serde_json::json!({"share_id": id, "parent_share":parent, "scope":scope}),
        )
        .unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "pendant/manifest.json", data.as_slice())
            .unwrap();
        tar.into_inner().unwrap().finish().unwrap();
        path
    }

    #[test]
    fn parent_lineage_sparse_membership_and_duplicate_scope_controls() {
        let root = tempfile::tempdir().unwrap();
        assert!(crate::pendant_sync::git_command(root.path())
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success());
        fs::create_dir_all(root.path().join("packages/api")).unwrap();
        fs::write(root.path().join("packages/api/source.rs"), "fixture").unwrap();
        assert!(crate::pendant_sync::git_command(root.path())
            .args(["add", "packages/api/source.rs"])
            .status()
            .unwrap()
            .success());
        let parent = archive(
            root.path(),
            "parent",
            "parent-id",
            Some("previous-parent-id"),
            serde_json::json!({"kind":"graph", "value":"urn:source"}),
        );
        let scope = |p| serde_json::json!({"kind":"construct", "value":query("urn:source", "urn:path", p).unwrap()});
        let api = archive(
            root.path(),
            "api",
            "api-id",
            Some("parent-id"),
            scope("packages/api/"),
        );
        let absent = archive(
            root.path(),
            "absent",
            "absent-id",
            Some("parent-id"),
            scope("packages/absent/"),
        );
        let inputs = vec![
            ("parent".into(), parent.clone()),
            ("api".into(), api.clone()),
            ("absent".into(), absent.clone()),
        ];
        let selected = select(root.path(), inputs).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].0, "api");
        assert!(crate::pendant_sync::git_command(root.path())
            .args(["update-index", "--skip-worktree", "packages/api/source.rs"])
            .status()
            .unwrap()
            .success());
        fs::remove_file(root.path().join("packages/api/source.rs")).unwrap();
        fs::create_dir(root.path().join("packages/api/node_modules")).unwrap();
        fs::write(
            root.path().join("packages/api/node_modules/cache"),
            "retained",
        )
        .unwrap();
        assert!(!checked_out(root.path(), "packages/api/").unwrap());
        assert_eq!(
            fs::read_to_string(root.path().join("packages/api/node_modules/cache")).unwrap(),
            "retained"
        );
        let duplicate = archive(
            root.path(),
            "duplicate",
            "different-id",
            Some("parent-id"),
            scope("packages/api/"),
        );
        assert!(select(
            root.path(),
            vec![
                ("parent".into(), parent.clone()),
                ("api".into(), api.clone()),
                ("duplicate".into(), duplicate)
            ]
        )
        .is_err());
        assert!(select(root.path(), vec![("api".into(), api)]).is_err());
        let mut budget = 16;
        assert!(manifest(&parent, &mut budget).is_err());
        assert!(select(root.path(), Vec::new()).unwrap().is_empty());
    }

    #[test]
    fn scopes_round_trip_and_cannot_escape_or_broaden() {
        for name in ["packages/client/", "projects/a b/", "projects/quote\"/"] {
            let value = query("urn:source", "urn:path", name).unwrap();
            let scope = Scope {
                kind: "construct".into(),
                value: Some(value.clone()),
            };
            assert_eq!(prefix(&scope).unwrap(), name);
            let broad = Scope {
                kind: "construct".into(),
                value: Some(value.replace(")) } }", ")) } } UNION { ?s ?p ?o }")),
            };
            assert!(prefix(&broad).is_err());
        }
        for name in [
            "",
            "/",
            "/absolute/",
            "../escape/",
            "a/../b/",
            "a//",
            "a//b/",
            "a/./",
            "a\\b/",
            "a\n/",
        ] {
            assert!(valid_prefix(name).is_err(), "{name:?}");
        }
        assert!(query("urn:source> } UNION {", "urn:path", "a/").is_err());
    }

    #[test]
    fn project_presence_tracks_checkout_and_rejects_redirects() {
        let root = tempfile::tempdir().unwrap();
        assert!(!present(root.path(), "packages/api/").unwrap());
        fs::create_dir_all(root.path().join("packages/api")).unwrap();
        assert!(present(root.path(), "packages/api/").unwrap());
        fs::remove_dir(root.path().join("packages/api")).unwrap();
        fs::write(root.path().join("packages/api"), "not a directory").unwrap();
        assert!(present(root.path(), "packages/api/").is_err());
        #[cfg(unix)]
        {
            fs::remove_file(root.path().join("packages/api")).unwrap();
            std::os::unix::fs::symlink(root.path(), root.path().join("packages/api")).unwrap();
            assert!(present(root.path(), "packages/api/").is_err());
        }
    }
}
