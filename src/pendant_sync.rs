//! Repository pendant discovery. Trust first; composition owns verification and CURRENT.
use crate::pack_trust::{self, Repo, TrustConfig};
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

const MAX_ARCHIVES: usize = 64;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const AUTHORITY_KEY: &str = "_caboodle-authority";

pub struct Options<'a> {
    pub repo: &'a Path,
    pub db: Option<&'a Path>,
    pub shapes_from: Option<&'a Path>,
    pub trust_file: Option<&'a Path>,
    pub monorepo: bool,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub outcome: &'static str,
    pub packs: usize,
    pub current: Option<serde_json::Value>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stamp {
    bytes: u64,
    modified: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u8,
    #[serde(default, skip_serializing_if = "is_false")]
    monorepo: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    selected: Vec<String>,
    repo: String,
    db: PathBuf,
    hashes: BTreeMap<String, String>,
    authority: String,
    database: Option<Stamp>,
    wal: Option<Stamp>,
}

fn is_false(value: &bool) -> bool {
    !value
}

pub(crate) fn git_command(root: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(root);
    // A hook may export these for its own repository; --repo remains authoritative.
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
    ] {
        command.env_remove(key);
    }
    command
}

pub(crate) fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = git_command(root).args(args).output()?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?
        .trim_end_matches(['\r', '\n'])
        .into())
}

fn sidecar(db: &Path, suffix: &str) -> Result<PathBuf> {
    let mut name = db
        .file_name()
        .context("database needs a file name")?
        .to_os_string();
    name.push(suffix);
    Ok(db.with_file_name(name))
}

fn stamp(path: &Path) -> Result<Option<Stamp>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
        Ok(m) => {
            if !m.is_file() || m.file_type().is_symlink() {
                bail!("expected a regular file: {}", path.display());
            }
            Ok(Some(Stamp {
                bytes: m.len(),
                modified: format!("{:?}", m.modified()?),
            }))
        }
    }
}

fn regular_directory(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => Ok(true),
        Ok(_) => bail!(
            "pendant directory must not be a symlink: {}",
            path.display()
        ),
    }
}

fn discover(root: &Path) -> Result<Vec<PathBuf>> {
    let quipu = root.join(".quipu");
    let packs = quipu.join("packs");
    if !regular_directory(&quipu)? || !regular_directory(&packs)? {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    for entry in fs::read_dir(&packs)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().context("pendant file name must be UTF-8")?;
        if !name.ends_with(".pendant.tar.gz") {
            continue;
        }
        if name.len() > 200
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            bail!("invalid pendant file name: {name}");
        }
        if !entry.file_type()?.is_file() {
            bail!("pendant must be a regular file, not a symlink: {name}");
        }
        found.push(entry.path());
        if found.len() > MAX_ARCHIVES {
            bail!("more than {MAX_ARCHIVES} pendants in {}", packs.display());
        }
    }
    found.sort();
    Ok(found)
}

/// Freeze the bytes that trust and compose both consume. The caller owns the authority path.
fn freeze(source: &Path, target: &Path, budget: &mut u64) -> Result<String> {
    if !fs::symlink_metadata(source)?.is_file() {
        bail!("not a regular pendant: {}", source.display());
    }
    let mut input = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        *budget = budget
            .checked_sub(n as u64)
            .context("pendant bytes exceed the 256 MiB sync limit")?;
        output.write_all(&buffer[..n])?;
        hash.update(&buffer[..n]);
    }
    output.sync_all()?;
    Ok(format!("{:x}", hash.finalize()))
}

pub fn sync(options: &Options<'_>) -> Result<Report> {
    let root = PathBuf::from(git(options.repo, &["rev-parse", "--show-toplevel"])?);
    let directory = PathBuf::from(git(&root, &["rev-parse", "--absolute-git-dir"])?);
    let default_db = directory.join("caboodle-pendants/store.db");
    let db = options.db.unwrap_or(&default_db);
    let db = if db.is_absolute() {
        db.to_path_buf()
    } else {
        std::env::current_dir()?.join(db)
    };
    let receipt_path = sidecar(&db, ".caboodle-packs.json")?;
    let packs = discover(&root)?;
    if packs.is_empty() && !receipt_path.exists() {
        return Ok(Report {
            outcome: "no_packs",
            packs: 0,
            current: None,
        });
    }
    let remote = git(&root, &["remote", "get-url", "origin"])?;
    let repo = Repo::parse(&remote)?.key();
    let authority = options.shapes_from.context(
        "packs sync requires --shapes-from pointing to an operator-controlled authority pendant",
    )?;
    let trust_path = match options.trust_file {
        Some(p) => p.to_path_buf(),
        None => pack_trust::default_path()?,
    };
    let trust = TrustConfig::load(&trust_path)?;
    let frozen = tempfile::tempdir()?;
    let mut budget = MAX_BYTES;
    let mut hashes = BTreeMap::new();
    let mut inputs = Vec::new();
    for path in &packs {
        let name = path.file_name().unwrap().to_str().unwrap();
        let target = frozen.path().join(name);
        let hash = freeze(path, &target, &mut budget)?;
        match trust.decide_pack(&remote, name, &hash) {
            pack_trust::Decision::Allow { .. } => (),
            pack_trust::Decision::Refuse { message } => bail!("{message}"),
        }
        hashes.insert(name.into(), hash);
        inputs.push((name.to_owned(), target));
    }
    let all_inputs = inputs.clone();
    if options.monorepo {
        inputs = crate::pendant_projects::select(&root, inputs)?;
    }
    let loaded_packs = inputs.len();
    fs::create_dir(frozen.path().join("operator"))?;
    let authority_copy = frozen.path().join("operator/authority.pendant.tar.gz");
    let authority_hash = freeze(authority, &authority_copy, &mut budget)?;
    let parent = db.parent().context("database needs a parent directory")?;
    fs::create_dir_all(parent)?;
    let lock_path = sidecar(&db, ".caboodle-packs.lock")?;
    if fs::symlink_metadata(&lock_path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        bail!("invalid pendant lock file");
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    lock.try_lock_exclusive()
        .context("another pendant sync owns this database; no compose attempted")?;
    let wal = sidecar(&db, "-wal")?;
    let mut receipt = Receipt {
        version: if options.monorepo { 2 } else { 1 },
        monorepo: options.monorepo,
        selected: if options.monorepo {
            inputs.iter().map(|(key, _)| key.clone()).collect()
        } else {
            Vec::new()
        },
        repo: repo.clone(),
        db: db.clone(),
        hashes,
        authority: authority_hash,
        database: stamp(&db)?,
        wal: stamp(&wal)?,
    };
    if receipt_path.exists() {
        let prior: Receipt = serde_json::from_slice(&fs::read(&receipt_path)?)
            .context("invalid pendant receipt; no compose attempted")?;
        if !matches!(prior.version, 1 | 2) {
            bail!("unsupported pendant receipt version; no compose attempted");
        }
        if receipt.database.is_some() && prior == receipt {
            return Ok(Report {
                outcome: "unchanged",
                packs: loaded_packs,
                current: None,
            });
        }
    }
    let program = std::env::var_os("QUIPU_BIN").unwrap_or_else(|| "quipu".into());
    if options.monorepo {
        // Even excluded parents and absent projects must pass native integrity,
        // vocabulary and authority gates. Verification touches only a throwaway store.
        let mut verify = Command::new(&program);
        verify
            .arg("compose")
            .arg("--db")
            .arg(frozen.path().join("verification.db"))
            .arg("--shapes-from")
            .arg(&authority_copy)
            .arg(&authority_copy);
        for (_, path) in &all_inputs {
            verify.arg(path);
        }
        let checked = verify.output().context("verify all monorepo shares")?;
        let report: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap_or_default();
        if !checked.status.success() || report["outcome"] != "composed" {
            bail!(
                "monorepo share verification refused; CURRENT and receipt unchanged: {}",
                String::from_utf8_lossy(&checked.stderr)
            );
        }
    }
    let mut command = Command::new(program);
    command
        .args(["compose", "--repo", &repo])
        .arg("--db")
        .arg(&db)
        .args(["--shapes-from", AUTHORITY_KEY]);
    command.arg(format!("{AUTHORITY_KEY}={}", authority_copy.display()));
    for (key, path) in inputs {
        command.arg(format!("{key}={}", path.display()));
    }
    let output = command
        .output()
        .context("run Quipu repository composition")?;
    if !output.status.success() {
        bail!(
            "pendant composition refused; receipt unchanged: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let result: serde_json::Value = serde_json::from_slice(&output.stdout)
        .context("Quipu returned no verifiable repository composition receipt")?;
    let expected = format!("urn:quipu:repo:{repo}:current");
    if result["repo"] != repo
        || result["composition"]["outcome"] != "composed"
        || result["current"]["dataset"] != expected
    {
        bail!(
            "Quipu did not attest the expected repository CURRENT composition; receipt unchanged"
        );
    }
    receipt.database = stamp(&db)?;
    receipt.wal = stamp(&wal)?;
    if receipt.database.is_none() {
        bail!("Quipu reported composition but the database is absent; receipt unchanged");
    }
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut file, &receipt)?;
    file.as_file().sync_all()?;
    file.persist(&receipt_path).context(
        "composition succeeded but saving its receipt failed; inspect CURRENT before retrying",
    )?;
    let saved: Receipt = serde_json::from_slice(&fs::read(&receipt_path)?)?;
    if saved != receipt {
        bail!("pendant receipt read-back differs");
    }
    Ok(Report {
        outcome: "composed",
        packs: loaded_packs,
        current: Some(result["current"].clone()),
    })
}
