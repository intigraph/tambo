//! Relative Quipu command alias; collision checks are read-only.
use std::{env, ffi::OsStr, fs, path::Path};

type Result<T> = std::result::Result<T, String>;

fn existing_alias(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("cannot inspect {}: {e}", path.display())),
        Ok(meta) => {
            if meta.file_type().is_symlink()
                && fs::read_link(path).ok().as_deref() == Some(Path::new("quipu"))
                && path.parent().is_some_and(|p| p.join("quipu").is_file())
            {
                Ok(())
            } else {
                Err(format!("qp collision: refusing existing {}; only a relative qp -> quipu alias is accepted", path.display()))
            }
        }
    }
}

fn preflight_path(bin: &Path, path: Option<&OsStr>) -> Result<()> {
    existing_alias(&bin.join("qp"))?;
    if let Some(path) = path {
        for directory in env::split_paths(path) {
            let candidate = directory.join("qp");
            if candidate.is_file() {
                existing_alias(&candidate)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn preflight(bin: &Path) -> Result<()> {
    preflight_path(bin, env::var_os("PATH").as_deref())
}

pub(crate) fn ensure(bin: &Path) -> Result<()> {
    preflight(bin)?;
    let target = bin.join("quipu");
    if !target.is_file() {
        return Err(format!("install {} before creating qp", target.display()));
    }
    let alias = bin.join("qp");
    if fs::symlink_metadata(&alias).is_err() {
        #[cfg(unix)]
        std::os::unix::fs::symlink("quipu", &alias).map_err(|e| {
            format!(
                "create {} without replacing existing files: {e}",
                alias.display()
            )
        })?;
        #[cfg(not(unix))]
        return Err("relative qp aliases currently require a Unix host".into());
    }
    let link = fs::read_link(&alias).map_err(|e| format!("read {}: {e}", alias.display()))?;
    let resolved_alias =
        fs::canonicalize(&alias).map_err(|e| format!("resolve {}: {e}", alias.display()))?;
    let resolved_target =
        fs::canonicalize(&target).map_err(|e| format!("resolve {}: {e}", target.display()))?;
    if link != Path::new("quipu") || resolved_alias != resolved_target {
        return Err("qp alias identity read-back failed".into());
    }
    Ok(())
}

/// What a post-install repair found and did to `qp` beside an owned `quipu`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Repair {
    NoQuipu,
    Current,
    Created,
    Retargeted,
    ForeignPreserved,
    /// quipu resolves to a file a package manager installed; its package owns
    /// the aliases, so nothing here is repaired.
    PackageManaged(String),
}

/// Self-healing for hosts that never run member apply (aegis-61f). Unlike
/// `ensure`, a symlink with the wrong target (or a dangling one) is replaced,
/// because a link named qp beside our quipu is ours to correct. Anything else
/// (a regular file, a directory) is foreign: it is reported and left alone.
#[cfg(unix)]
pub(crate) fn repair(bin: &Path) -> Result<Repair> {
    repair_with(bin, crate::apt_owned::owner)
}

#[cfg(unix)]
fn repair_with(bin: &Path, package_owner: impl Fn(&Path) -> Option<String>) -> Result<Repair> {
    let target = bin.join("quipu");
    if !target.is_file() {
        return Ok(Repair::NoQuipu);
    }
    // is_file() follows symlinks, so ask dpkg about the RESOLVED quipu.
    if let Some(package) = package_owner(&target) {
        return Ok(Repair::PackageManaged(package));
    }
    let alias = bin.join("qp");
    let outcome = match fs::symlink_metadata(&alias) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::os::unix::fs::symlink("quipu", &alias)
                .map_err(|e| format!("create {}: {e}", alias.display()))?;
            Repair::Created
        }
        Err(e) => return Err(format!("cannot inspect {}: {e}", alias.display())),
        Ok(meta) if !meta.file_type().is_symlink() => return Ok(Repair::ForeignPreserved),
        Ok(_) if fs::read_link(&alias).ok().as_deref() == Some(Path::new("quipu")) => {
            Repair::Current
        }
        Ok(_) => {
            // Build the new link beside the old one and rename over it, so qp
            // is never absent and never a half-written file. Never clear the
            // staging name first: whatever is already there is not provably
            // ours, so creation refuses (AlreadyExists) and the repair fails.
            let staged = bin.join(format!(".qp.caboodle-{}", std::process::id()));
            std::os::unix::fs::symlink("quipu", &staged)
                .map_err(|e| format!("stage {}: {e}", staged.display()))?;
            fs::rename(&staged, &alias).map_err(|e| {
                let _ = fs::remove_file(&staged);
                format!("replace {}: {e}", alias.display())
            })?;
            Repair::Retargeted
        }
    };
    let resolved_alias =
        fs::canonicalize(&alias).map_err(|e| format!("resolve {}: {e}", alias.display()))?;
    let resolved_target =
        fs::canonicalize(&target).map_err(|e| format!("resolve {}: {e}", target.display()))?;
    if fs::read_link(&alias).ok().as_deref() != Some(Path::new("quipu"))
        || resolved_alias != resolved_target
    {
        return Err("qp alias identity read-back failed".into());
    }
    Ok(outcome)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn fixture() -> std::path::PathBuf {
        let dir = env::temp_dir().join(format!(
            "qp-alias-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        dir
    }

    #[test]
    fn fresh_relative_alias_is_idempotent_and_tracks_replacement() {
        let dir = fixture();
        fs::write(dir.join("quipu"), b"old").unwrap();
        ensure(&dir).unwrap();
        ensure(&dir).unwrap();
        assert_eq!(fs::read_link(dir.join("qp")).unwrap(), Path::new("quipu"));
        fs::write(dir.join("next"), b"new").unwrap();
        fs::rename(dir.join("next"), dir.join("quipu")).unwrap();
        assert_eq!(fs::read(dir.join("qp")).unwrap(), b"new");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn foreign_destination_and_dangling_link_are_preserved() {
        let dir = fixture();
        fs::write(dir.join("qp"), b"foreign").unwrap();
        assert!(preflight_path(&dir, None).is_err());
        assert_eq!(fs::read(dir.join("qp")).unwrap(), b"foreign");
        fs::remove_file(dir.join("qp")).unwrap();
        symlink("missing", dir.join("qp")).unwrap();
        assert!(preflight_path(&dir, None).is_err());
        assert_eq!(fs::read_link(dir.join("qp")).unwrap(), Path::new("missing"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn foreign_qp_anywhere_on_path_refuses_before_alias_creation() {
        let dir = fixture();
        let foreign = fixture();
        fs::write(foreign.join("qp"), b"foreign").unwrap();
        let path = env::join_paths([&foreign, &dir]).unwrap();
        assert!(preflight_path(&dir, Some(&path)).is_err());
        assert!(!dir.join("qp").exists());
        fs::remove_dir_all(dir).unwrap();
        fs::remove_dir_all(foreign).unwrap();
    }

    #[test]
    fn repair_creates_then_leaves_a_current_alias() {
        let dir = fixture();
        assert_eq!(repair(&dir).unwrap(), Repair::NoQuipu);
        assert!(fs::symlink_metadata(dir.join("qp")).is_err());
        fs::write(dir.join("quipu"), b"q").unwrap();
        assert_eq!(repair(&dir).unwrap(), Repair::Created);
        assert_eq!(repair(&dir).unwrap(), Repair::Current);
        assert_eq!(fs::read_link(dir.join("qp")).unwrap(), Path::new("quipu"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn repair_retargets_wrong_and_dangling_links_only() {
        let dir = fixture();
        fs::write(dir.join("quipu"), b"q").unwrap();
        fs::write(dir.join("other"), b"o").unwrap();
        for wrong in ["other", "missing", "/bin/sh"] {
            let _ = fs::remove_file(dir.join("qp"));
            symlink(wrong, dir.join("qp")).unwrap();
            assert_eq!(repair(&dir).unwrap(), Repair::Retargeted);
            assert_eq!(fs::read_link(dir.join("qp")).unwrap(), Path::new("quipu"));
            assert_eq!(fs::read(dir.join("other")).unwrap(), b"o");
        }
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .filter(|n| n.to_string_lossy().starts_with(".qp."))
            .collect();
        assert!(
            leftovers.is_empty(),
            "staged link left behind: {leftovers:?}"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn repair_refuses_rather_than_delete_a_foreign_staging_file() {
        let dir = fixture();
        fs::write(dir.join("quipu"), b"q").unwrap();
        symlink("missing", dir.join("qp")).unwrap();
        let staged = dir.join(format!(".qp.caboodle-{}", std::process::id()));
        fs::write(&staged, b"foreign marker").unwrap();
        assert!(repair(&dir).is_err());
        assert_eq!(fs::read(&staged).unwrap(), b"foreign marker");
        assert_eq!(fs::read_link(dir.join("qp")).unwrap(), Path::new("missing"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn repair_never_touches_a_foreign_file_or_directory() {
        let dir = fixture();
        fs::write(dir.join("quipu"), b"q").unwrap();
        fs::write(dir.join("qp"), b"foreign").unwrap();
        assert_eq!(repair(&dir).unwrap(), Repair::ForeignPreserved);
        assert_eq!(fs::read(dir.join("qp")).unwrap(), b"foreign");
        fs::remove_file(dir.join("qp")).unwrap();
        fs::create_dir(dir.join("qp")).unwrap();
        assert_eq!(repair(&dir).unwrap(), Repair::ForeignPreserved);
        assert!(dir.join("qp").is_dir());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn repair_leaves_a_package_managed_quipu_alone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("quipu"), "binary").unwrap();
        let resolved = std::fs::canonicalize(dir.path().join("quipu")).unwrap();
        let owner =
            |p: &Path| (std::fs::canonicalize(p).ok()? == resolved).then(|| "quipu".to_string());
        assert_eq!(
            repair_with(dir.path(), owner).unwrap(),
            Repair::PackageManaged("quipu".into())
        );
        assert!(
            std::fs::symlink_metadata(dir.path().join("qp")).is_err(),
            "no qp created"
        );
        // Not package-owned: the ordinary repair still creates the alias.
        assert_eq!(repair_with(dir.path(), |_| None).unwrap(), Repair::Created);
    }
}
