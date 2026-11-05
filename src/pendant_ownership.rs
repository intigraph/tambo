//! Resolve CODEOWNERS with Git's path matching, then build native RDF scopes.
use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    process::{Command, Stdio},
};

#[derive(Clone, Debug)]
pub struct OwnedPath {
    pub path: String,
    pub predicate: String,
    pub owner: String,
}

/// Native CONSTRUCT currently requires owner resources in the producer store.
pub fn verify_owners(graph: &str, db: &Path, owners: &[OwnedPath]) -> Result<()> {
    if !fs::symlink_metadata(db).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink()) {
        bail!("ownership generation needs an existing regular producer database");
    }
    let unique: std::collections::BTreeSet<_> = owners.iter().map(|o| o.owner.as_str()).collect();
    if unique.len() > 128 {
        bail!("ownership generation exceeds 128 distinct owners");
    }
    for owner in unique {
        let query = format!("ASK {{ GRAPH <{graph}> {{ <{owner}> ?p ?o }} }}");
        let program = std::env::var_os("QUIPU_BIN").unwrap_or_else(|| "quipu".into());
        let result = Command::new(program)
            .args(["read", &query, "--db"])
            .arg(db)
            .output()?;
        if !result.status.success() || String::from_utf8_lossy(&result.stdout).trim() != "true" {
            bail!("CODEOWNERS owner {owner} is absent or unreadable in the producer graph; register the resource before exporting ownership");
        }
    }
    Ok(())
}

fn owner_iri(owner: &str) -> Result<String> {
    if !owner.starts_with('@') {
        let (local, domain) = owner
            .split_once('@')
            .context("CODEOWNERS owner must be @user, @org/team or an email")?;
        if local.is_empty()
            || domain.is_empty()
            || !local
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ".-_+".contains(c))
            || !domain
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            bail!("unsupported CODEOWNERS email: {owner}");
        }
        return Ok(format!("mailto:{owner}"));
    }
    let name = owner
        .strip_prefix('@')
        .context("CODEOWNERS owners must be @user or @org/team")?;
    let parts: Vec<_> = name.split('/').collect();
    if parts.is_empty()
        || parts.len() > 2
        || parts.iter().any(|p| {
            p.is_empty()
                || !p
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
    {
        bail!("unsupported CODEOWNERS identity: {owner}");
    }
    Ok(if parts.len() == 1 {
        format!("https://github.com/{name}")
    } else {
        format!("https://github.com/orgs/{}/teams/{}", parts[0], parts[1])
    })
}

/// No owner is invented when CODEOWNERS is absent. GitHub's first-file
/// precedence and last matching rule apply; unsupported syntax refuses.
pub fn resolve(repo: &Path, prefix: &str, predicate: &str) -> Result<Vec<OwnedPath>> {
    crate::pendant_projects::valid_prefix(prefix)?;
    let root = std::path::PathBuf::from(crate::pendant_sync::git(
        repo,
        &["rev-parse", "--show-toplevel"],
    )?);
    let mut text = None;
    for name in [".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"] {
        let path = root.join(name);
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
            Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.len() <= 3 * 1024 * 1024 => {
                if let Some(parent) = Path::new(name)
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                {
                    if !crate::pendant_projects::present(&root, &format!("{}/", parent.display()))?
                    {
                        bail!("CODEOWNERS parent absent");
                    }
                }
                text = Some(fs::read_to_string(path)?);
                break;
            }
            Ok(_) => bail!("CODEOWNERS must be a regular file no larger than 3 MiB"),
        }
    }
    let Some(text) = text else {
        return Ok(Vec::new());
    };
    let listed = crate::pendant_sync::git_command(&root)
        .args(["--literal-pathspecs", "ls-files", "-z", "--", prefix])
        .output()?;
    if !listed.status.success() {
        bail!("cannot enumerate tracked project paths");
    }
    let paths = String::from_utf8(listed.stdout)?;
    let paths: Vec<_> = paths.split('\0').filter(|s| !s.is_empty()).collect();
    if paths.len() > 2048 {
        bail!("CODEOWNERS scope exceeds 2048 tracked project paths");
    }
    let scratch = tempfile::tempdir()?;
    let git = |args: &[&str]| {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(scratch.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_COMMON_DIR")
            .args([
                "-c",
                "core.excludesFile=/dev/null",
                "-c",
                "core.ignoreCase=false",
            ])
            .args(args);
        cmd
    };
    if !git(&["init", "-q"]).status()?.success() {
        bail!("cannot initialize CODEOWNERS matching fixture");
    }
    let mut owners: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut rule_count = 0;
    for line in text.lines() {
        let fields: Vec<_> = line
            .split_whitespace()
            .take_while(|f| !f.starts_with('#'))
            .collect();
        if fields.is_empty() {
            continue;
        }
        rule_count += 1;
        if rule_count > 1024 {
            bail!("CODEOWNERS exceeds 1024 rules");
        }
        let pattern = fields[0];
        if pattern.starts_with('!') || pattern.contains(['[', ']', '\\']) {
            bail!("unsupported CODEOWNERS pattern: {pattern}");
        }
        let values = fields[1..]
            .iter()
            .map(|s| owner_iri(s))
            .collect::<Result<Vec<_>>>()?;
        fs::write(scratch.path().join(".gitignore"), format!("{pattern}\n"))?;
        let input_path = scratch.path().join("paths");
        fs::write(&input_path, format!("{}\0", paths.join("\0")))?;
        let matched = git(&["check-ignore", "--no-index", "-z", "--stdin"])
            .stdin(Stdio::from(fs::File::open(input_path)?))
            .output()?;
        if !matches!(matched.status.code(), Some(0 | 1)) {
            bail!(
                "Git refused CODEOWNERS matching: {}",
                String::from_utf8_lossy(&matched.stderr)
            );
        }
        for path in String::from_utf8(matched.stdout)?
            .split('\0')
            .filter(|s| !s.is_empty())
        {
            if !paths.contains(&path) {
                bail!("Git returned a path outside the project");
            }
            owners.insert(path.into(), values.clone());
        }
    }
    let mut result = Vec::new();
    for (path, owners) in owners {
        for owner in owners {
            result.push(OwnedPath {
                path: path.clone(),
                predicate: predicate.into(),
                owner,
            });
        }
    }
    if result.len() > 4096 {
        bail!("ownership scope exceeds 4096 owner assignments");
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codeowners_uses_precedence_last_match_and_explicit_unowned_rule() {
        let root = tempfile::tempdir().unwrap();
        assert!(Command::new("git")
            .args(["init", "-q"])
            .arg(root.path())
            .status()
            .unwrap()
            .success());
        fs::create_dir_all(root.path().join("packages/api/private")).unwrap();
        fs::create_dir(root.path().join(".github")).unwrap();
        for name in [
            "packages/api/a.rs",
            "packages/api/private/b.rs",
            "packages/api/free.rs",
        ] {
            fs::write(root.path().join(name), "fixture").unwrap();
        }
        assert!(Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args(["add", "."])
            .status()
            .unwrap()
            .success());
        fs::write(root.path().join("CODEOWNERS"), "* @wrong\n").unwrap();
        fs::write(root.path().join(".github/CODEOWNERS"), "* @fallback\n/packages/api/ @org/api @lead\n/packages/api/private/ @private\n/packages/api/free.rs\n").unwrap();
        let values = resolve(root.path(), "packages/api/", "urn:owned_by").unwrap();
        assert_eq!(values.len(), 3);
        assert!(values
            .iter()
            .any(|v| v.path == "packages/api/a.rs"
                && v.owner == "https://github.com/orgs/org/teams/api"));
        assert!(values
            .iter()
            .any(|v| v.path == "packages/api/a.rs" && v.owner == "https://github.com/lead"));
        assert!(values
            .iter()
            .any(|v| v.path == "packages/api/private/b.rs"
                && v.owner == "https://github.com/private"));
        assert!(!values.iter().any(|v| v.path.ends_with("free.rs")));
        fs::write(root.path().join(".github/CODEOWNERS"), "!secret @owner\n").unwrap();
        assert!(resolve(root.path(), "packages/api/", "urn:owned_by").is_err());
    }
    #[test]
    fn ownership_scope_round_trip_and_outside_path_refusal() {
        let owners = vec![OwnedPath {
            path: "packages/api/a.rs".into(),
            predicate: "urn:owned_by".into(),
            owner: owner_iri("@org/api").unwrap(),
        }];
        let value = crate::pendant_projects::enriched_query(
            "urn:source",
            "urn:path",
            "packages/api/",
            &owners,
        )
        .unwrap();
        let scope = crate::pendant_projects::Scope {
            kind: "construct".into(),
            value: Some(value.clone()),
        };
        assert_eq!(
            crate::pendant_projects::prefix(&scope).unwrap(),
            "packages/api/"
        );
        let scope = crate::pendant_projects::Scope {
            kind: "construct".into(),
            value: Some(value.replace("UNION { GRAPH", "OPTIONAL { GRAPH")),
        };
        assert!(crate::pendant_projects::prefix(&scope).is_err());
        assert!(crate::pendant_projects::enriched_query(
            "urn:source",
            "urn:path",
            "other/",
            &owners
        )
        .is_err());
    }
}
