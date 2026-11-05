//! Fixed-path immutable job runtime delivery; no identity/sudo/readiness edits.
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
};

const ROOT: &str = "/opt/shantytown-job-launcher";
const HELPER: &str = "/usr/local/libexec/st-job-launcher";
const MAX_FILE: u64 = 268_435_456;
const MAX_TOTAL: u64 = 1_073_741_824;
#[cfg(target_os = "macos")]
const NOFOLLOW: i32 = 0x100;
#[cfg(not(target_os = "macos"))]
const NOFOLLOW: i32 = 0x20000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    platform: String,
    files: BTreeMap<String, String>,
}
#[derive(Debug, Serialize)]
pub struct Plan {
    pub changed: bool,
    pub manifest_sha256: String,
    pub platform: String,
    pub files: usize,
    pub bytes: u64,
}
struct Verified {
    plan: Plan,
    manifest: Manifest,
    raw: Vec<u8>,
}

fn hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn regular(path: &Path) -> Result<u64> {
    let info = fs::symlink_metadata(path)?;
    if !info.is_file() || info.len() > MAX_FILE {
        bail!("runtime input must be a bounded regular file");
    }
    Ok(info.len())
}
fn hash(path: &Path) -> Result<String> {
    regular(path)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(NOFOLLOW)
        .open(path)?;
    let mut digest = Sha256::new();
    let mut total = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > MAX_FILE {
            bail!("runtime input grew beyond its bound");
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
        && (name == "launcher"
            || [
                "runtime/bin/python3",
                "runtime/bin/sd",
                "runtime/bin/codex",
                "runtime/bin/claude",
            ]
            .contains(&name)
            || name.starts_with("runtime/app/shantytown/")
            || name.starts_with("runtime/lib/"))
}
fn inventory(root: &Path) -> Result<(BTreeSet<String>, BTreeSet<String>)> {
    let mut todo = vec![root.to_path_buf()];
    let (mut files, mut dirs) = (BTreeSet::new(), BTreeSet::new());
    while let Some(directory) = todo.pop() {
        if !fs::symlink_metadata(&directory)?.is_dir() {
            bail!("runtime directory required");
        }
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .strip_prefix(root)?
                .to_str()
                .context("non-UTF8 runtime path")?
                .to_string();
            let info = fs::symlink_metadata(&path)?;
            if info.is_dir() {
                dirs.insert(name);
                todo.push(path);
            } else if info.is_file() {
                files.insert(name);
            } else {
                bail!("runtime symlink or special file refused");
            }
            if files.len() + dirs.len() > 8192 {
                bail!("runtime inventory exceeds its bound");
            }
        }
    }
    Ok((files, dirs))
}
fn parents(files: &BTreeSet<String>) -> BTreeSet<String> {
    files
        .iter()
        .flat_map(|name| {
            Path::new(name)
                .ancestors()
                .skip(1)
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.to_str().unwrap().to_string())
        })
        .collect()
}
fn load(directory: &Path, expected: &str) -> Result<Verified> {
    if !hex(expected) {
        bail!("explicit reviewed manifest SHA256 required");
    }
    let manifest_path = directory.join("manifest.json");
    if regular(&manifest_path)? > 1_048_576 || hash(&manifest_path)? != expected {
        bail!("runtime manifest differs from its reviewed pin");
    }
    let raw = fs::read(manifest_path)?;
    if raw.len() > 1_048_576 || format!("{:x}", Sha256::digest(&raw)) != expected {
        bail!("runtime manifest changed during preparation");
    }
    let manifest: Manifest = serde_json::from_slice(&raw)?;
    if manifest.version != 1
        || !["linux", "macos"].contains(&manifest.platform.as_str())
        || manifest.files.len() > 4096
        || manifest
            .files
            .iter()
            .any(|(name, sha)| !safe_name(name) || !hex(sha))
    {
        bail!("invalid explicit native runtime manifest");
    }
    for required in [
        "launcher",
        "runtime/bin/python3",
        "runtime/app/shantytown/__init__.py",
        "runtime/app/shantytown/job_launch.py",
        "runtime/app/shantytown/job_metadata.py",
    ] {
        if !manifest.files.contains_key(required) {
            bail!("runtime artifact is incomplete");
        }
    }
    let libraries: Vec<_> = manifest
        .files
        .keys()
        .filter_map(|name| {
            let minor = name
                .strip_prefix("runtime/lib/python3.")?
                .strip_suffix("/os.py")?;
            (minor.parse::<u32>().ok()? >= 11).then_some(minor)
        })
        .collect();
    if libraries.len() != 1
        || !manifest.files.contains_key(&format!(
            "runtime/lib/python3.{}/encodings/__init__.py",
            libraries[0]
        ))
    {
        bail!("one private Python >=3.11 standard library is required");
    }
    let mut expected_files: BTreeSet<String> = manifest.files.keys().cloned().collect();
    expected_files.insert("manifest.json".into());
    if inventory(directory)? != (expected_files.clone(), parents(&expected_files)) {
        bail!("runtime artifact differs from its exact manifest inventory");
    }
    let mut bytes = 0;
    for (name, sha) in &manifest.files {
        bytes += regular(&directory.join(name))?;
        if bytes > MAX_TOTAL || hash(&directory.join(name))? != *sha {
            bail!("runtime artifact bytes differ from the reviewed manifest");
        }
    }
    let launcher = fs::read(directory.join("launcher"))?;
    if !launcher.starts_with(b"#!/opt/shantytown-job-launcher/bin/python3 -BIS\n") {
        bail!("launcher must use the fixed isolated no-bytecode interpreter");
    }
    Ok(Verified {
        plan: Plan {
            changed: false,
            manifest_sha256: expected.into(),
            platform: manifest.platform.clone(),
            files: manifest.files.len(),
            bytes,
        },
        manifest,
        raw,
    })
}

pub fn preview(directory: &Path, expected: &str) -> Result<Plan> {
    Ok(load(directory, expected)?.plan)
}
fn protected(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        bail!("absolute protected runtime path required");
    }
    for part in path.ancestors() {
        let info = fs::symlink_metadata(part)?;
        if info.uid() != 0 || info.mode() & 0o022 != 0 || (!info.is_dir() && !info.is_file()) {
            bail!("runtime path is mutable or foreign");
        }
    }
    Ok(())
}
fn directory(path: &Path) -> Result<()> {
    protected(path.parent().context("runtime directory has no parent")?)?;
    if !path.exists() {
        fs::DirBuilder::new().mode(0o755).create(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    protected(path)?;
    if !path.is_dir() {
        bail!("protected directory required");
    }
    Ok(())
}
fn sync(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}
fn copy(source: &Path, destination: &Path, executable: bool, expected: &str) -> Result<()> {
    let mut input = OpenOptions::new()
        .read(true)
        .custom_flags(NOFOLLOW)
        .open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(NOFOLLOW)
        .open(destination)?;
    std::io::copy(
        &mut Read::by_ref(&mut input).take(MAX_FILE + 1),
        &mut output,
    )?;
    output.sync_all()?;
    if hash(destination)? != expected {
        bail!("runtime changed while copying; retain staging");
    }
    fs::set_permissions(
        destination,
        fs::Permissions::from_mode(if executable { 0o755 } else { 0o644 }),
    )?;
    output.sync_all()?;
    Ok(())
}
fn verify_installed(root: &Path, verified: &Verified) -> Result<()> {
    protected(root)?;
    let mut expected: BTreeSet<String> = verified
        .manifest
        .files
        .keys()
        .filter_map(|name| name.strip_prefix("runtime/").map(str::to_string))
        .collect();
    expected.insert(".artifact.json".into());
    if inventory(root)? != (expected.clone(), parents(&expected)) {
        bail!("installed runtime inventory changed");
    }
    for name in expected {
        let path = root.join(&name);
        protected(&path)?;
        let wanted = if name == ".artifact.json" {
            &verified.plan.manifest_sha256
        } else {
            &verified.manifest.files[&format!("runtime/{name}")]
        };
        if hash(&path)? != *wanted {
            bail!("installed runtime differs; explicit recovery required");
        }
    }
    Ok(())
}

/// New immutable installation or exact same-artifact readback. Never an upgrade.
pub fn apply(source: &Path, expected: &str) -> Result<Plan> {
    crate::job_slots::require_root()?;
    protected(source)?;
    let mut verified = load(source, expected)?;
    if verified.plan.platform != std::env::consts::OS {
        bail!("runtime artifact is for another platform");
    }
    protected(&source.join("manifest.json"))?;
    for name in verified.manifest.files.keys() {
        protected(&source.join(name))?;
    }
    directory(Path::new("/opt"))?;
    directory(Path::new("/usr/local/libexec"))?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(NOFOLLOW)
        .open("/opt/.shantytown-job-runtime.lock")?;
    let info = lock.metadata()?;
    if !info.is_file() || info.uid() != 0 || info.mode() & 0o077 != 0 {
        bail!("unsafe runtime installation lock");
    }
    lock.try_lock_exclusive()?;
    let helper = Path::new(HELPER);
    if helper.exists() || helper.is_symlink() {
        protected(helper)?;
        if hash(helper)? != verified.manifest.files["launcher"] {
            bail!("different helper installed; explicit recovery required");
        }
    }
    let root = Path::new(ROOT);
    verified.plan.changed = !root.exists() || !helper.exists();
    if root.exists() || root.is_symlink() {
        verify_installed(root, &verified)?;
    } else {
        let staging = Path::new("/opt/.shantytown-job-launcher.pending");
        fs::DirBuilder::new().mode(0o755).create(staging)?; // retain on every later failure
        fs::set_permissions(staging, fs::Permissions::from_mode(0o755))?;
        let files: BTreeSet<String> = verified
            .manifest
            .files
            .keys()
            .filter_map(|name| name.strip_prefix("runtime/").map(str::to_string))
            .collect();
        let dirs = parents(&files);
        for relative in &dirs {
            directory(&staging.join(relative))?;
        }
        for relative in files {
            copy(
                &source.join("runtime").join(&relative),
                &staging.join(&relative),
                relative.starts_with("bin/"),
                &verified.manifest.files[&format!("runtime/{relative}")],
            )?;
        }
        let receipt = staging.join(".artifact.json");
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&receipt)?;
        output.write_all(&verified.raw)?;
        output.sync_all()?;
        verify_installed(staging, &verified)?;
        for relative in dirs.iter().rev() {
            sync(&staging.join(relative))?;
        }
        sync(staging)?;
        fs::rename(staging, root)?;
        sync(Path::new("/opt"))?;
    }
    if !helper.exists() {
        let staged = helper.with_file_name(".st-job-launcher.pending");
        copy(
            &source.join("launcher"),
            &staged,
            true,
            &verified.manifest.files["launcher"],
        )?;
        fs::rename(staged, helper)?;
        sync(helper.parent().unwrap())?;
    }
    protected(helper)?;
    if hash(helper)? != verified.manifest.files["launcher"] {
        bail!("helper readback differs");
    }
    verify_installed(root, &verified)?;
    Ok(verified.plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn save(root: &Path, files: &BTreeMap<String, String>) -> String {
        let raw = serde_json::to_vec(
            &serde_json::json!({"version":1,"platform":std::env::consts::OS,"files":files}),
        )
        .unwrap();
        fs::write(root.join("manifest.json"), &raw).unwrap();
        format!("{:x}", Sha256::digest(raw))
    }
    fn fixture() -> (tempfile::TempDir, BTreeMap<String, String>, String) {
        let root = tempfile::tempdir().unwrap();
        let mut files = BTreeMap::new();
        for name in [
            "launcher",
            "runtime/bin/python3",
            "runtime/app/shantytown/__init__.py",
            "runtime/app/shantytown/job_launch.py",
            "runtime/app/shantytown/job_metadata.py",
            "runtime/lib/python3.11/os.py",
            "runtime/lib/python3.11/encodings/__init__.py",
        ] {
            let path = root.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(
                &path,
                if name == "launcher" {
                    b"#!/opt/shantytown-job-launcher/bin/python3 -BIS\n# fixture, never executed\n"
                        .as_slice()
                } else {
                    b"synthetic inert fixture".as_slice()
                },
            )
            .unwrap();
            files.insert(name.to_string(), hash(&path).unwrap());
        }
        let pin = save(root.path(), &files);
        (root, files, pin)
    }
    #[test]
    fn preview_requires_exact_manifest_and_never_executes_fixture_bytes() {
        let (root, files, pin) = fixture();
        let plan = preview(root.path(), &pin).unwrap();
        assert_eq!(plan.files, files.len());
        assert_eq!(plan.manifest_sha256, pin);
        assert!(preview(root.path(), &"0".repeat(64)).is_err());
        fs::write(root.path().join("runtime/bin/python3"), "changed").unwrap();
        assert!(preview(root.path(), &pin).is_err());
    }
    #[test]
    fn optional_trusted_client_and_native_harnesses_are_explicitly_scoped() {
        let (root, mut files, _) = fixture();
        for name in ["sd", "codex", "claude"] {
            let relative = format!("runtime/bin/{name}");
            let path = root.path().join(&relative);
            fs::write(&path, "inert native fixture, never executed").unwrap();
            files.insert(relative, hash(&path).unwrap());
        }
        let pin = save(root.path(), &files);
        let plan = preview(root.path(), &pin).unwrap();
        assert_eq!(plan.files, files.len());
        assert!(!plan.changed);
    }
    #[test]
    fn extra_missing_or_symlinked_artifact_member_refuses() {
        for fault in ["extra", "missing", "symlink"] {
            let (root, _, pin) = fixture();
            let launcher = root.path().join("launcher");
            match fault {
                "extra" => fs::write(root.path().join("extra"), "unexpected").unwrap(),
                "missing" => fs::remove_file(launcher).unwrap(),
                _ => {
                    fs::remove_file(&launcher).unwrap();
                    symlink("/usr/bin/true", launcher).unwrap();
                }
            }
            assert!(preview(root.path(), &pin).is_err(), "{fault}");
        }
    }
    #[test]
    fn traversal_absolute_and_unscoped_paths_refuse_even_with_matching_manifest_pin() {
        for name in [
            "/absolute",
            "runtime/../escape",
            "runtime//bin/other",
            "runtime/bin/shell",
            "runtime/app/foreign/code.py",
        ] {
            let (root, mut files, _) = fixture();
            files.insert(name.into(), "0".repeat(64));
            let pin = save(root.path(), &files);
            assert!(preview(root.path(), &pin).is_err(), "{name}");
        }
    }
    #[test]
    fn application_and_isolated_interpreter_are_mandatory() {
        for name in [
            "runtime/bin/python3",
            "runtime/app/shantytown/job_metadata.py",
            "runtime/lib/python3.11/os.py",
        ] {
            let (root, mut files, _) = fixture();
            files.remove(name);
            let pin = save(root.path(), &files);
            assert!(preview(root.path(), &pin).is_err());
        }
        let (root, mut files, _) = fixture();
        let path = root.path().join("launcher");
        fs::write(&path, "#!/usr/bin/env python3\n").unwrap();
        files.insert("launcher".into(), hash(&path).unwrap());
        let pin = save(root.path(), &files);
        assert!(preview(root.path(), &pin).is_err());
    }
    #[test]
    fn copy_is_exclusive_and_mismatch_retains_nonexecutable_staging() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::write(&source, "fixture").unwrap();
        let pin = hash(&source).unwrap();
        copy(&source, &target, true, &pin).unwrap();
        assert_eq!(hash(&target).unwrap(), pin);
        assert_eq!(fs::metadata(&target).unwrap().mode() & 0o777, 0o755);
        assert!(copy(&source, &target, true, &pin).is_err());
        let bad = root.path().join("retained");
        assert!(copy(&source, &bad, true, &"0".repeat(64)).is_err());
        assert_eq!(fs::metadata(&bad).unwrap().mode() & 0o777, 0o600);
        assert_eq!(fs::read(&bad).unwrap(), b"fixture");
    }
}
