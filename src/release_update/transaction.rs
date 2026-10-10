//! Durable rollback point for an updater interrupted between rename and verification.
use super::{atomic_copy, durable, hash};
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
    durable::write(&journal(state), &serde_json::to_vec(pending)?)
}

pub(super) fn finish(state: &Path) -> Result<()> {
    let target = journal(state);
    let bytes = fs::read(&target)?;
    fs::remove_file(&target).context("clear completed release transaction")?;
    let cleared = (|| {
        durable::checkpoint("journal-unlinked", &target)?;
        durable::sync_dir(durable::parent(&target))?;
        durable::checkpoint("journal-clear-durable", &target)
    })();
    if let Err(error) = cleared {
        // Do not certify an unlink whose persistence is unknown. Retain the
        // exact pending record when possible, so a later invocation recovers.
        let retained = durable::write(&target, &bytes);
        return Err(error).context(format!(
            "journal clear durability uncertain; pending retention: {retained:?}"
        ));
    }
    Ok(())
}

pub(super) fn recover(state: &Path) -> Result<()> {
    let journal = journal(state);
    match fs::symlink_metadata(&journal) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context("cannot establish pending release journal state"),
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => {
            bail!("pending release journal must be a regular non-symlink file; refusing recovery")
        }
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
    fs::File::open(state)?.sync_all()?;
    durable::sync_dir(durable::parent(state))?;
    finish(state)?;
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
    durable::write(&target, &serde_json::to_vec(pending)?)?;
    durable::checkpoint("pair-journal-durable", &target)
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
        durable::sync_dir(durable::parent(&artifact.destination))?;
    }
    match &pending.previous_state {
        Some(bytes) => durable::write(state, bytes)?,
        None => match fs::remove_file(state) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        },
    }
    durable::sync_dir(durable::parent(state))?;
    finish(state)?;
    println!("quipu: restored interrupted/failed paired release and prior state");
    Ok(())
}

#[cfg(test)]
mod journal_tests {
    use super::*;

    #[test]
    fn dangling_or_nonregular_pending_journals_refuse_and_remain() {
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("state.json");
        let journal = state.with_extension("release-pending.json");
        std::os::unix::fs::symlink(root.path().join("missing"), &journal).unwrap();
        assert!(recover(&state)
            .unwrap_err()
            .to_string()
            .contains("regular non-symlink"));
        assert!(fs::symlink_metadata(&journal)
            .unwrap()
            .file_type()
            .is_symlink());
        fs::remove_file(&journal).unwrap();
        fs::create_dir(&journal).unwrap();
        assert!(recover(&state).is_err());
        assert!(journal.is_dir());
        fs::remove_dir(&journal).unwrap();
        recover(&state).unwrap();
    }
}

#[cfg(test)]
mod ordered_tests {
    use super::super::durable::probe;
    use super::*;

    fn fixture(root: &Path) -> (PathBuf, PairPending) {
        let state = root.join("state.json");
        fs::write(&state, b"prior exact state").unwrap();
        let bin = root.join("bin");
        fs::create_dir(&bin).unwrap();
        let mut artifacts = Vec::new();
        for (name, bytes) in [
            ("quipu", b"prior CLI".as_slice()),
            ("quipu-server", b"prior server".as_slice()),
        ] {
            let destination = bin.join(name);
            fs::write(&destination, bytes).unwrap();
            let backup = root
                .join("release-backups")
                .join(name)
                .join(hash(&destination).unwrap());
            artifacts.push(Pending {
                tool: "quipu".into(),
                sha256: hash(&destination).unwrap(),
                destination,
                backup,
            });
        }
        (
            state,
            PairPending {
                artifacts,
                previous_state: Some(b"prior exact state".to_vec()),
            },
        )
    }

    #[test]
    fn new_ancestors_and_backup_renames_are_synced_before_journal_and_clear_before_success() {
        let root = tempfile::tempdir().unwrap();
        let (state, pair) = fixture(root.path());
        probe::start(None);
        for a in &pair.artifacts {
            atomic_copy(&a.destination, &a.backup).unwrap();
        }
        begin_pair(&state, &pair).unwrap();
        finish(&state).unwrap();
        let trace = probe::take().events;
        let journal_at = trace
            .iter()
            .position(|(s, _)| *s == "pair-journal-durable")
            .unwrap();
        for (i, (stage, path)) in trace.iter().enumerate() {
            if *stage == "directory-created" {
                assert!(trace[i + 1..journal_at]
                    .iter()
                    .any(|(s, p)| *s == "directory-synced" && p == path));
                assert!(trace[i + 1..journal_at]
                    .iter()
                    .any(|(s, p)| *s == "directory-synced" && p == durable::parent(path)));
            }
            if *stage == "copy-renamed" {
                assert!(i < journal_at);
                assert!(trace[i + 1..journal_at]
                    .iter()
                    .any(|(s, p)| *s == "directory-synced" && p == durable::parent(path)));
            }
        }
        assert_eq!(
            trace.iter().filter(|(s, _)| *s == "copy-renamed").count(),
            2
        );
        let unlink = trace
            .iter()
            .position(|(s, _)| *s == "journal-unlinked")
            .unwrap();
        let cleared = trace
            .iter()
            .position(|(s, _)| *s == "journal-clear-durable")
            .unwrap();
        assert!(journal_at < unlink && unlink < cleared);
        assert!(trace[unlink + 1..cleared]
            .iter()
            .any(|(s, p)| *s == "directory-synced" && p == durable::parent(&state)));
    }

    #[test]
    fn failed_ancestor_or_backup_barrier_never_arms_journal_and_retry_syncs_residue() {
        for fault in ["ancestor", "backup"] {
            let root = tempfile::tempdir().unwrap();
            let (state, pair) = fixture(root.path());
            let path = if fault == "ancestor" {
                root.path().join("release-backups")
            } else {
                durable::parent(&pair.artifacts[0].backup).to_path_buf()
            };
            probe::start(Some(("directory-sync", path.clone())));
            assert!(
                atomic_copy(&pair.artifacts[0].destination, &pair.artifacts[0].backup).is_err()
            );
            let trace = probe::take().events;
            assert!(!state.with_extension("release-pending.json").exists());
            assert!(!trace.iter().any(|(s, _)| *s == "pair-journal-durable"));
            // The failed mkdir's entry now EXISTS. It must still be synced on retry.
            probe::start(None);
            for a in &pair.artifacts {
                atomic_copy(&a.destination, &a.backup).unwrap();
            }
            begin_pair(&state, &pair).unwrap();
            let trace = probe::take().events;
            let ready = trace
                .iter()
                .position(|(s, _)| *s == "pair-journal-durable")
                .unwrap();
            assert!(trace[..ready]
                .iter()
                .any(|(s, p)| *s == "directory-synced" && p == &path));
        }
    }

    #[test]
    fn failed_clear_barrier_retains_exact_journal_for_later_recovery_not_success() {
        let root = tempfile::tempdir().unwrap();
        let (state, pair) = fixture(root.path());
        for a in &pair.artifacts {
            atomic_copy(&a.destination, &a.backup).unwrap();
        }
        begin_pair(&state, &pair).unwrap();
        let original = fs::read(journal(&state)).unwrap();
        for a in &pair.artifacts {
            fs::write(&a.destination, b"new unconfirmed").unwrap();
        }
        fs::write(&state, b"new unconfirmed state").unwrap();
        probe::start(Some(("directory-sync", root.path().to_path_buf())));
        assert!(finish(&state)
            .unwrap_err()
            .to_string()
            .contains("durability uncertain"));
        let trace = probe::take().events;
        assert!(!trace.iter().any(|(s, _)| *s == "journal-clear-durable"));
        assert_eq!(fs::read(journal(&state)).unwrap(), original);
        recover(&state).unwrap();
        for a in &pair.artifacts {
            assert_eq!(hash(&a.destination).unwrap(), a.sha256);
        }
        assert_eq!(fs::read(&state).unwrap(), b"prior exact state");
        assert!(!journal(&state).exists());
    }

    #[test]
    fn old_single_artifact_journal_recovers_without_paired_fields() {
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("state.json");
        let mut value = State::read(&state).unwrap();
        value.tools.insert(
            "bobbin".into(),
            crate::model::ToolState {
                version: "keeper".into(),
                applied: true,
                verified: true,
            },
        );
        value.tools.insert(
            "seeds".into(),
            crate::model::ToolState {
                version: "unconfirmed".into(),
                applied: true,
                verified: true,
            },
        );
        value.write(&state).unwrap();
        let destination = root.path().join("sd");
        let backup = root.path().join("backup");
        fs::write(&destination, b"new").unwrap();
        fs::write(&backup, b"old").unwrap();
        let legacy = serde_json::json!({"tool":"seeds","destination":destination,"backup":backup,"sha256":hash(&backup).unwrap()});
        fs::write(journal(&state), serde_json::to_vec(&legacy).unwrap()).unwrap();
        recover(&state).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"old");
        let restored = State::read(&state).unwrap();
        assert_eq!(restored.tools["bobbin"].version, "keeper");
        assert!(!restored.tools.contains_key("seeds"));
        assert!(!journal(&state).exists());
    }
}
