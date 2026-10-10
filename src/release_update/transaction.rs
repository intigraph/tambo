//! Durable rollback point for an updater interrupted between rename and verification.
use super::{atomic_copy, hash};
use crate::model::State;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Deserialize, Serialize)]
pub(super) struct Pending {
    pub tool: String,
    pub destination: PathBuf,
    pub backup: PathBuf,
    pub sha256: String,
}

fn journal(state: &Path) -> PathBuf {
    state.with_extension("release-pending.json")
}

pub(super) fn begin(state: &Path, pending: &Pending) -> Result<()> {
    let target = journal(state);
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file = tempfile::NamedTempFile::new_in(parent)?;
    fs::write(file.path(), serde_json::to_vec(pending)?)?;
    file.as_file().sync_all()?;
    file.persist(target).map_err(|e| e.error)?;
    Ok(())
}

pub(super) fn finish(state: &Path) -> Result<()> {
    fs::remove_file(journal(state)).context("clear completed release transaction")
}

pub(super) fn recover(state: &Path) -> Result<()> {
    let journal = journal(state);
    if !journal.exists() {
        return Ok(());
    }
    let bytes = fs::read(&journal)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    if value.get("artifacts").is_some() {
        let pair: PairPending = serde_json::from_slice(&bytes)?;
        return restore_pair(state, &pair);
    }
    let pending: Pending = serde_json::from_slice(&bytes)?;
    if hash(&pending.backup)? != pending.sha256 {
        bail!("pending rollback backup checksum mismatch; refusing recovery");
    }
    atomic_copy(&pending.backup, &pending.destination)
        .context("restore interrupted release update")?;
    if hash(&pending.destination)? != pending.sha256 {
        bail!("interrupted release rollback read-back mismatch");
    }
    // A previous state write might have completed before the process died. It
    // cannot certify the restored binary; make it earn functional proof again.
    let mut previous = State::read(state)?;
    previous.tools.remove(&pending.tool);
    previous.write(state)?;
    fs::remove_file(journal)?;
    println!(
        "{}: recovered interrupted update from {}",
        pending.tool,
        pending.backup.display()
    );
    Ok(())
}

/// Paired releases share the SAME lock and pending journal as single binaries.
/// Keep the old format readable; a pair must restore all artifacts and exact state.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PairPending {
    pub artifacts: Vec<Pending>,
    pub previous_state: Option<Vec<u8>>,
}

pub(super) fn begin_pair(state: &Path, pending: &PairPending) -> Result<()> {
    if pending.artifacts.len() != 2 {
        bail!("paired release must contain exactly two artifacts");
    }
    let target = journal(state);
    let file = tempfile::NamedTempFile::new_in(state.parent().context("state directory")?)?;
    fs::write(file.path(), serde_json::to_vec(pending)?)?;
    file.as_file().sync_all()?;
    file.persist(target).map_err(|e| e.error)?;
    fs::File::open(state.parent().context("state directory")?)?.sync_all()?;
    Ok(())
}

fn restore_pair(state: &Path, pending: &PairPending) -> Result<()> {
    if pending.artifacts.len() != 2
        || pending.artifacts[0].destination == pending.artifacts[1].destination
    {
        bail!("invalid paired release journal");
    }
    // Validate ALL backup bytes before restoring any of them.
    for artifact in &pending.artifacts {
        for path in [&artifact.backup, &artifact.destination] {
            if !fs::symlink_metadata(path)?.file_type().is_file() {
                bail!("paired recovery requires regular non-symlink files");
            }
        }
        if hash(&artifact.backup)? != artifact.sha256 {
            bail!("paired rollback backup checksum mismatch; refusing recovery");
        }
    }
    for artifact in &pending.artifacts {
        atomic_copy(&artifact.backup, &artifact.destination)?;
        if hash(&artifact.destination)? != artifact.sha256 {
            bail!("paired rollback read-back mismatch");
        }
        fs::File::open(artifact.destination.parent().context("binary directory")?)?.sync_all()?;
    }
    match &pending.previous_state {
        Some(bytes) => {
            let file = tempfile::NamedTempFile::new_in(state.parent().context("state directory")?)?;
            fs::write(file.path(), bytes)?;
            file.as_file().sync_all()?;
            file.persist(state).map_err(|e| e.error)?;
        }
        None => match fs::remove_file(state) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        },
    }
    fs::File::open(state.parent().context("state directory")?)?.sync_all()?;
    finish(state)?;
    println!("quipu: restored interrupted/failed paired release and prior state");
    Ok(())
}
