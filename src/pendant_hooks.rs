//! Explicit opt-in only. Existing or redirected Git hooks are never replaced.
use crate::pendant_sync::{git, git_command, Options};
use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn install(options: &Options<'_>) -> Result<Vec<PathBuf>> {
    let root = PathBuf::from(git(options.repo, &["rev-parse", "--show-toplevel"])?);
    // core.hooksPath can be shared by unrelated repositories; do not change its custody.
    let configured = git_command(&root)
        .args(["config", "--get", "core.hooksPath"])
        .output()?;
    if !configured.status.success() && configured.status.code() != Some(1) {
        bail!("cannot determine core.hooksPath; no hooks installed");
    }
    if configured.status.success() {
        bail!("core.hooksPath is configured; run packs sync from the existing hook instead");
    }
    let directory = PathBuf::from(git(&root, &["rev-parse", "--absolute-git-dir"])?);
    let hooks = PathBuf::from(git(&root, &["rev-parse", "--git-path", "hooks"])?);
    let hooks = if hooks.is_absolute() {
        hooks
    } else {
        root.join(hooks)
    };
    if fs::symlink_metadata(&hooks).is_ok_and(|m| !m.is_dir() || m.file_type().is_symlink()) {
        bail!("Git hook directory is not a regular directory");
    }
    let authority = fs::canonicalize(
        options
            .shapes_from
            .context("--pack-hooks requires --shapes-from")?,
    )?;
    let db = options
        .db
        .map(Path::to_path_buf)
        .unwrap_or_else(|| directory.join("caboodle-pendants/store.db"));
    let db = if db.is_absolute() {
        db
    } else {
        std::env::current_dir()?.join(db)
    };
    let executable = std::env::current_exe()?;
    let mut line = format!(
        "exec {} packs sync --repo \"$root\" --shapes-from {} --db {}",
        quote(
            executable
                .to_str()
                .context("executable path must be UTF-8")?
        ),
        quote(authority.to_str().context("authority path must be UTF-8")?),
        quote(db.to_str().context("database path must be UTF-8")?)
    );
    if let Some(path) = options.trust_file {
        let path = fs::canonicalize(path)?;
        line.push_str(&format!(
            " --trust-file {}",
            quote(path.to_str().context("trust path must be UTF-8")?)
        ));
    }
    let body = format!("#!/bin/sh\n# caboodle pendant sync: installed by explicit init --pack-hooks\nset -eu\nroot=$(git rev-parse --show-toplevel)\n{line}\n");
    let paths: Vec<_> = ["post-checkout", "post-merge"]
        .map(|n| hooks.join(n))
        .into();
    // Preflight both hooks before creating either. Partial I/O failures remain visible.
    for path in &paths {
        if let Ok(meta) = fs::symlink_metadata(path) {
            if !meta.is_file() || meta.file_type().is_symlink() || fs::read_to_string(path)? != body
            {
                bail!("existing hook retained: {}", path.display());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if meta.permissions().mode() & 0o111 == 0 {
                    bail!("matching hook is not executable: {}", path.display());
                }
            }
        }
    }
    fs::create_dir_all(&hooks)?;
    for path in &paths {
        if path.exists() {
            continue;
        }
        let mut file = tempfile::NamedTempFile::new_in(&hooks)?;
        file.write_all(body.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file()
                .set_permissions(fs::Permissions::from_mode(0o755))?;
        }
        file.as_file().sync_all()?;
        file.persist_noclobber(path).with_context(|| {
            format!(
                "hook installation incomplete at {}; existing files were retained",
                path.display()
            )
        })?;
    }
    Ok(paths)
}
