//! Tools installed by a system package manager are not Tambo's to replace.
//!
//! A host can install a stack tool from the signed apt repository and keep its
//! user path (for example `~/.local/bin/sd`) as a symlink to the packaged
//! `/usr/bin/sd`. Replacing that path would turn the symlink into a regular
//! file and silently shadow the package: apt would keep upgrading a binary
//! nobody runs. So before replacing or re-aliasing a binary, Tambo resolves
//! the path and asks dpkg who owns the result. An owned binary is refused with
//! the package name, and the package manager stays the only updater.
//!
//! A host without dpkg (macOS, other distributions) owns nothing here, so this
//! check changes nothing there.

use std::fs;
use std::path::Path;
use std::process::Command;

/// The dpkg package owning the file `path` resolves to, if any.
#[must_use]
pub fn owner(path: &Path) -> Option<String> {
    let program = std::env::var_os("CABOODLE_DPKG_QUERY").unwrap_or_else(|| "dpkg-query".into());
    owner_with(Path::new(&program), path)
}

fn owner_with(program: &Path, path: &Path) -> Option<String> {
    let resolved = fs::canonicalize(path).ok()?;
    let out = Command::new(program)
        .arg("-S")
        .arg(&resolved)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let wanted = resolved.to_string_lossy();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|line| {
            // "package[:arch][, other]: /path". Diversion lines name no owner.
            let (packages, file) = line.split_once(": ")?;
            if line.starts_with("diversion by") || file != wanted {
                return None;
            }
            let first = packages.split(", ").next()?;
            Some(first.split(':').next()?.to_string())
        })
}

/// Refuse to replace a package-owned binary, naming the package to use instead.
pub fn refuse_replacement(tool: &str, path: &Path) -> anyhow::Result<()> {
    refuse_if_owned(tool, path, owner(path))
}

fn refuse_if_owned(tool: &str, path: &Path, owner: Option<String>) -> anyhow::Result<()> {
    if let Some(package) = owner {
        anyhow::bail!(
            "{tool}: {} is managed by apt (package {package}); refusing to replace it. \
             Update it with `sudo apt install --only-upgrade {package}`, or roll back with \
             `sudo apt install {package}=<version>`",
            path.display()
        );
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn fake_dpkg_query(dir: &Path, body: &str) -> std::path::PathBuf {
        let path = dir.join("dpkg-query");
        crate::test_exec::write_executable(&path, &format!("#!/bin/sh\n{body}\n"));
        path
    }

    #[test]
    fn a_symlink_to_a_package_file_is_owned_by_that_package() {
        let dir = tempfile::tempdir().unwrap();
        let packaged = dir.path().join("usr-bin-sd");
        fs::write(&packaged, "binary").unwrap();
        let link = dir.path().join("sd");
        symlink(&packaged, &link).unwrap();
        let resolved = fs::canonicalize(&packaged).unwrap();
        let query = fake_dpkg_query(
            dir.path(),
            &format!(
                "[ \"$2\" = '{}' ] && echo 'seeds:amd64: {}' && exit 0; exit 1",
                resolved.display(),
                resolved.display()
            ),
        );
        assert_eq!(owner_with(&query, &link).as_deref(), Some("seeds"));
        let error = refuse_if_owned("seeds", &link, owner_with(&query, &link))
            .unwrap_err()
            .to_string();
        assert!(error.contains("managed by apt (package seeds)"), "{error}");
        // A plain user file is not owned, so replacement stays allowed.
        let plain = dir.path().join("plain");
        fs::write(&plain, "binary").unwrap();
        assert_eq!(owner_with(&query, &plain), None);
        assert!(refuse_if_owned("plain", &plain, owner_with(&query, &plain)).is_ok());
        // No dpkg at all (macOS): nothing is owned.
        assert_eq!(owner_with(&dir.path().join("absent"), &link), None);
    }

    #[test]
    fn diversions_and_other_paths_are_not_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x");
        fs::write(&file, "x").unwrap();
        let resolved = fs::canonicalize(&file).unwrap();
        let query = fake_dpkg_query(
            dir.path(),
            &format!(
                "echo 'diversion by foo from: {0}'; echo 'other: {0}.bak'; exit 0",
                resolved.display()
            ),
        );
        assert_eq!(owner_with(&query, &file), None);
    }
}
