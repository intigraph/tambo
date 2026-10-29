//! Explicit macOS job accounts from Shantytown's reviewed slot inventory.
//! No key/token enrollment, helper install, readiness toggle, or UID recycling.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    version: u32,
    uid_min: u32,
    uid_max: u32,
    slots: BTreeMap<String, Reservation>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reservation {
    uid: u32,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Account {
    pub principal: String,
    pub username: String,
    pub uid: u32,
    pub home: String,
    pub manifest_sha256: String,
}
#[derive(Debug, Serialize)]
pub struct Plan {
    pub uid_min: u32,
    pub uid_max: u32,
    pub accounts: Vec<Account>,
}

fn hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn principal(value: &str) -> bool {
    value
        .strip_prefix("eph-")
        .and_then(|v| v.rsplit_once('-'))
        .is_some_and(|(kind, suffix)| {
            ["review", "merge", "triage", "task"].contains(&kind) && hex(suffix, 6)
        })
}
fn bounded(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.len() > maximum {
        bail!("slot input is not a bounded regular file");
    }
    let raw = fs::read(path)?;
    if raw.len() as u64 > maximum {
        bail!("slot input grew beyond its bound");
    }
    Ok(raw)
}
fn native_id(value: &str) -> Result<u32> {
    let value: i64 = value.parse()?;
    if !(i32::MIN as i64..=u32::MAX as i64).contains(&value) {
        bail!("native UID/GID outside uid_t range");
    }
    // Directory Services spells nobody as -2; the kernel's uid_t is u32.
    Ok(value as u32)
}
fn not_admin(answer: &str, username: &str) -> bool {
    answer.trim() == format!("no {username} is NOT a member of admin")
}
fn absent(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error.into()),
    }
}

/// Read-only preparation, also usable off macOS. Manifest bytes must match the
/// exact reviewed catalog; only account metadata is emitted, never argv/env.
pub fn load(directory: &Path) -> Result<Plan> {
    let catalog: Catalog =
        serde_json::from_slice(&bounded(&directory.join("slots.json"), 1_048_576)?)?;
    if catalog.version != 1
        || catalog.uid_min < 10000
        || catalog.uid_max < catalog.uid_min
        || catalog.slots.is_empty()
        || catalog.slots.len() > 1024
    {
        bail!("explicit bounded UID slot plan required");
    }
    let actual: BTreeSet<String> = fs::read_dir(directory)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .filter(|n| n.starts_with("eph-") && n.ends_with(".json"))
        .map(|n| n.strip_suffix(".json").unwrap_or(&n).to_owned())
        .collect();
    if actual != catalog.slots.keys().cloned().collect() {
        bail!("manifest set differs from reviewed slots");
    }
    let mut uids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut controllers = BTreeSet::new();
    let mut accounts = Vec::new();
    for (name, slot) in catalog.slots {
        if !principal(&name)
            || !(catalog.uid_min..=catalog.uid_max).contains(&slot.uid)
            || !hex(&slot.sha256, 64)
            || !uids.insert(slot.uid)
        {
            bail!("invalid or reused slot UID/principal");
        }
        let raw = bounded(&directory.join(format!("{name}.json")), 65536)?;
        if format!("{:x}", Sha256::digest(&raw)) != slot.sha256 {
            bail!("manifest digest differs from reviewed slot");
        }
        let value: Value = serde_json::from_slice(&raw)?;
        let username = value["username"]
            .as_str()
            .context("slot username missing")?;
        let home = format!("/var/lib/shantytown/agents/crew-{name}");
        let controller = value["controller_uid"]
            .as_u64()
            .context("controller UID missing")?;
        if value["version"] != 1
            || value["principal"] != name
            || value["uid"] != slot.uid
            || value["gid"] != slot.uid
            || value["home"] != home
            || value["root"] != format!("/var/lib/shantytown/job-stores/{name}")
            || controller == 0
            || controller > u32::MAX as u64
            || controller == slot.uid as u64
            || !(username == "_stjobs"
                || username.strip_prefix("_stjobs_").is_some_and(|s| hex(s, 6)))
            || !names.insert(username.to_owned())
        {
            bail!("slot account identity is not isolated");
        }
        controllers.insert(controller as u32);
        accounts.push(Account {
            principal: name,
            username: username.to_owned(),
            uid: slot.uid,
            home,
            manifest_sha256: slot.sha256,
        });
    }
    if controllers.iter().any(|uid| uids.contains(uid)) {
        bail!("job UID cannot be a controller");
    }
    Ok(Plan {
        uid_min: catalog.uid_min,
        uid_max: catalog.uid_max,
        accounts,
    })
}

/// Fixed native commands only. No shell, caller executable, or arbitrary user
/// property is accepted. Separate per-job primary group, disabled login, hidden.
pub fn commands(account: &Account) -> Vec<Vec<String>> {
    let group = format!("/Groups/{}", account.username);
    let user = format!("/Users/{}", account.username);
    let ds = |target: &str, property: &str, value: &str| {
        vec![
            "/usr/bin/dscl".into(),
            ".".into(),
            "-create".into(),
            target.into(),
            property.into(),
            value.into(),
        ]
    };
    vec![
        ds(&group, "PrimaryGroupID", &account.uid.to_string()),
        ds(&group, "Password", "*"),
        ds(&user, "UniqueID", &account.uid.to_string()),
        ds(&user, "PrimaryGroupID", &account.uid.to_string()),
        ds(&user, "NFSHomeDirectory", &account.home),
        ds(&user, "UserShell", "/usr/bin/false"),
        ds(&user, "Password", "*"),
        ds(&user, "AuthenticationAuthority", ";DisabledUser;"),
        ds(&user, "IsHidden", "1"),
    ]
}

#[cfg(unix)]
mod native {
    use super::*;
    use fs2::FileExt;
    use std::{
        fs::{File, OpenOptions},
        io::{Read, Seek},
        os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };
    const BASE: &str = "/Library/Application Support/Shantytown/jobs";
    const NOFOLLOW: i32 = 0x100; // Darwin SDK sys/fcntl.h O_NOFOLLOW

    fn protected(path: &Path) -> Result<()> {
        for entry in path.ancestors() {
            let meta = fs::symlink_metadata(entry)?;
            if meta.uid() != 0 || meta.mode() & 0o022 != 0 || (!meta.is_dir() && !meta.is_file()) {
                bail!("account plan path is not immutable root-owned input");
            }
        }
        Ok(())
    }
    fn run(args: &[String]) -> Result<(bool, String)> {
        // Regular private capture files cannot wait for EOF from a descendant
        // retaining a pipe after the fixed native command has exited.
        let mut stdout = tempfile::tempfile()?;
        let stderr = tempfile::tempfile()?;
        let mut child = Command::new(&args[0])
            .args(&args[1..])
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout.try_clone()?))
            .stderr(Stdio::from(stderr.try_clone()?))
            .spawn()?;
        let start = Instant::now();
        let status = loop {
            let oversized =
                stdout.metadata()?.len() > 1_048_576 || stderr.metadata()?.len() > 1_048_576;
            if oversized || start.elapsed() > Duration::from_secs(10) {
                let _ = child.kill();
                child.wait()?;
                bail!("native account command exceeded time/byte budget; preserve reservation");
            }
            if let Some(status) = child.try_wait()? {
                break status;
            }
            thread::sleep(Duration::from_millis(20));
        };
        stdout.rewind()?;
        let mut bytes = Vec::new();
        stdout.take(1_048_577).read_to_end(&mut bytes)?;
        if bytes.len() > 1_048_576 || stderr.metadata()?.len() > 1_048_576 {
            bail!("native account answer exceeded byte budget");
        }
        Ok((status.success(), String::from_utf8(bytes)?))
    }

    #[test]
    fn inherited_output_descriptor_does_not_block_native_readback() {
        let started = Instant::now();
        let answer = run(&[
            "/bin/sh".into(),
            "-c".into(),
            "sleep 3 & printf fixture".into(),
        ]);
        let elapsed = started.elapsed();
        // Let this fixture's own descendant finish even if an assertion fails.
        thread::sleep(Duration::from_secs(3));
        assert_eq!(answer.unwrap(), (true, "fixture".into()));
        assert!(
            elapsed < Duration::from_secs(1),
            "readback waited {elapsed:?}"
        );
    }
    fn inventory(kind: &str, property: &str) -> Result<BTreeMap<String, u32>> {
        let (ok, text) = run(&[
            "/usr/bin/dscl".into(),
            ".".into(),
            "-list".into(),
            kind.into(),
            property.into(),
        ])?;
        if !ok || text.trim().is_empty() {
            bail!("native account inventory unknown");
        }
        let mut values = BTreeMap::new();
        for line in text.lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() != 2
                || values
                    .insert(fields[0].into(), native_id(fields[1])?)
                    .is_some()
            {
                bail!("native account inventory malformed");
            }
        }
        Ok(values)
    }
    fn check(account: &Account) -> Result<()> {
        let (ok, text) = run(&[
            "/usr/bin/dscl".into(),
            ".".into(),
            "-read".into(),
            format!("/Users/{}", account.username),
            "UniqueID".into(),
            "PrimaryGroupID".into(),
            "NFSHomeDirectory".into(),
            "UserShell".into(),
            "IsHidden".into(),
            "AuthenticationAuthority".into(),
        ])?;
        if !ok {
            bail!("job account readback unknown");
        }
        let mut fields = BTreeMap::new();
        for line in text.lines() {
            let (key, value) = line
                .split_once(':')
                .context("job account readback malformed")?;
            if fields.insert(key.trim(), value.trim()).is_some() {
                bail!("job account has multiple property values");
            }
        }
        for (key, expected) in [
            ("UniqueID", account.uid.to_string()),
            ("PrimaryGroupID", account.uid.to_string()),
            ("NFSHomeDirectory", account.home.clone()),
            ("UserShell", "/usr/bin/false".into()),
            ("IsHidden", "1".into()),
            ("AuthenticationAuthority", ";DisabledUser;".into()),
        ] {
            if fields.get(key) != Some(&expected.as_str()) {
                bail!("job account is only partially provisioned");
            }
        }
        let (member, answer) = run(&[
            "/usr/sbin/dseditgroup".into(),
            "-o".into(),
            "checkmember".into(),
            "-m".into(),
            account.username.clone(),
            "admin".into(),
        ])?;
        if member || !not_admin(&answer, &account.username) {
            bail!("job account administrative membership is not disproved");
        }
        let home = fs::symlink_metadata(&account.home)?;
        if !home.is_dir() || home.uid() != account.uid || home.mode() & 0o077 != 0 {
            bail!("job home is not private to its UID");
        }
        Ok(())
    }
    fn root_dir(path: &Path) -> Result<()> {
        if absent(path)? {
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o755).create(path)?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        }
        if !fs::symlink_metadata(path)?.is_dir() {
            bail!("root account path is not a directory");
        }
        protected(path)
    }
    fn save(path: &Path, account: &Account, state: &str, exclusive: bool) -> Result<()> {
        use std::io::Write;
        let raw = serde_json::to_vec(
            &serde_json::json!({"principal":account.principal,"uid":account.uid,"sha256":account.manifest_sha256,"state":state}),
        )?;
        if exclusive {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(NOFOLLOW)
                .open(path)?;
            file.write_all(&raw)?;
            file.sync_all()?;
        } else {
            protected(path)?;
            let mut file = tempfile::NamedTempFile::new_in(
                path.parent().context("reservation parent missing")?,
            )?;
            file.write_all(&raw)?;
            file.as_file().sync_all()?;
            file.persist(path)?;
        }
        File::open(path.parent().context("reservation parent missing")?)?.sync_all()?;
        Ok(())
    }
    pub fn apply(directory: &Path) -> Result<Plan> {
        if !cfg!(target_os = "macos") {
            bail!("macOS account apply requires native Darwin; Linux uses its IaC role");
        }
        let (root, uid) = run(&["/usr/bin/id".into(), "-u".into()])?;
        if !root || uid.trim() != "0" {
            bail!("account apply requires separately admitted root execution");
        }
        protected(&std::env::current_exe()?)?;
        if directory != Path::new(BASE) {
            bail!("account apply requires the fixed protected host inventory");
        }
        protected(directory)?;
        protected(&directory.join("slots.json"))?;
        let mut lock = OpenOptions::new();
        lock.read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(NOFOLLOW);
        let lock = lock.open(directory.join(".account-lock"))?;
        if lock.metadata()?.uid() != 0 || lock.metadata()?.mode() & 0o077 != 0 {
            bail!("invalid account reservation lock");
        }
        FileExt::try_lock_exclusive(&lock)?;
        let plan = load(directory)?;
        for account in &plan.accounts {
            protected(&directory.join(format!("{}.json", account.principal)))?;
        }
        let receipts = directory.join("account-receipts");
        root_dir(&receipts)?;
        let users = inventory("/Users", "UniqueID")?;
        let groups = inventory("/Groups", "PrimaryGroupID")?;
        let mut fresh = Vec::new();
        for account in &plan.accounts {
            let receipt = receipts.join(format!("{}.json", account.uid));
            if !absent(&receipt)? {
                protected(&receipt)?;
                let value: Value = serde_json::from_slice(&bounded(&receipt, 4096)?)?;
                if value
                    != serde_json::json!({"principal":account.principal,"uid":account.uid,"sha256":account.manifest_sha256,"state":"complete"})
                {
                    bail!("retained UID reservation needs explicit recovery; never recycle it");
                }
                if users.get(&account.username) != Some(&account.uid)
                    || groups.get(&account.username) != Some(&account.uid)
                    || users.values().filter(|uid| **uid == account.uid).count() != 1
                    || groups.values().filter(|uid| **uid == account.uid).count() != 1
                {
                    bail!("retained job identity differs from native accounts");
                }
                check(account)?;
            } else {
                if users.contains_key(&account.username)
                    || groups.contains_key(&account.username)
                    || users.values().any(|uid| *uid == account.uid)
                    || groups.values().any(|uid| *uid == account.uid)
                    || !absent(Path::new(&account.home))?
                {
                    bail!("slot account/UID/home is already occupied; no adoption");
                }
                fresh.push(account);
            }
        }
        // Reserve every UID durably before the first account change. Any later
        // error retains all prepared reservations, including an unused account.
        for account in &fresh {
            save(
                &receipts.join(format!("{}.json", account.uid)),
                account,
                "prepared",
                true,
            )?;
        }
        for path in [
            "/private/var",
            "/private/var/lib",
            "/private/var/lib/shantytown",
            "/private/var/lib/shantytown/agents",
        ] {
            root_dir(Path::new(path))?;
        }
        for account in fresh {
            for command in commands(account) {
                if !run(&command)?.0 {
                    bail!("native account creation failed; retain reservation");
                }
            }
            let canonical = PathBuf::from(format!(
                "/private/var/lib/shantytown/agents/crew-{}",
                account.principal
            ));
            fs::DirBuilder::new().mode(0o700).create(&canonical)?;
            fs::set_permissions(&canonical, fs::Permissions::from_mode(0o700))?;
            if !run(&[
                "/usr/sbin/chown".into(),
                format!("{}:{}", account.uid, account.uid),
                canonical.to_string_lossy().into_owned(),
            ])?
            .0
            {
                bail!("job home ownership unproved");
            }
            let users = inventory("/Users", "UniqueID")?;
            let groups = inventory("/Groups", "PrimaryGroupID")?;
            if users.get(&account.username) != Some(&account.uid)
                || groups.get(&account.username) != Some(&account.uid)
                || users.values().filter(|uid| **uid == account.uid).count() != 1
                || groups.values().filter(|uid| **uid == account.uid).count() != 1
            {
                bail!("job UID/GID uniqueness unproved");
            }
            check(account)?;
            save(
                &receipts.join(format!("{}.json", account.uid)),
                account,
                "complete",
                false,
            )?;
        }
        Ok(plan)
    }
}
#[cfg(unix)]
pub use native::apply;

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let name = "eph-task-123abc";
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"principal":name,"uid":51001,"gid":51001,
            "controller_uid":501,"username":"_stjobs_123abc","home":format!("/var/lib/shantytown/agents/crew-{name}"),
            "root":format!("/var/lib/shantytown/job-stores/{name}")})).unwrap();
        fs::write(root.path().join(format!("{name}.json")), &raw).unwrap();
        fs::write(
            root.path().join("slots.json"),
            serde_json::to_vec(
                &serde_json::json!({"version":1,"uid_min":51001,"uid_max":51016,
            "slots":{name:{"uid":51001,"sha256":format!("{:x}",Sha256::digest(&raw))}}}),
            )
            .unwrap(),
        )
        .unwrap();
        root
    }
    #[test]
    fn exact_plan_generates_only_disabled_hidden_own_account_commands() {
        let root = fixture();
        let plan = load(root.path()).unwrap();
        assert_eq!(plan.accounts.len(), 1);
        let cmds = commands(&plan.accounts[0]);
        assert!(cmds
            .iter()
            .all(|c| c[0] == "/usr/bin/dscl" && c[1] == "." && c[2] == "-create"));
        assert!(cmds.iter().any(|c| c[4] == "IsHidden" && c[5] == "1"));
        assert!(cmds
            .iter()
            .any(|c| c[4] == "UserShell" && c[5] == "/usr/bin/false"));
        assert!(cmds
            .iter()
            .all(|c| !c.iter().any(|v| v == "admin" || v == "sudo")));
    }
    #[test]
    fn changed_missing_or_extra_manifest_refuses_before_any_native_command() {
        for fault in ["changed", "missing", "extra"] {
            let root = fixture();
            let path = root.path().join("eph-task-123abc.json");
            match fault {
                "changed" => fs::write(path, "{}").unwrap(),
                "missing" => fs::remove_file(path).unwrap(),
                _ => fs::write(root.path().join("eph-task-ffffff.json"), "{}").unwrap(),
            }
            assert!(load(root.path()).is_err());
        }
    }
    #[test]
    fn low_unbounded_or_empty_uid_plan_refuses() {
        for (lo, hi, empty) in [
            (9999, 51016, false),
            (51001, 50000, false),
            (51001, 51016, true),
        ] {
            let root = fixture();
            let path = root.path().join("slots.json");
            let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            value["uid_min"] = lo.into();
            value["uid_max"] = hi.into();
            if empty {
                value["slots"] = serde_json::json!({});
            }
            fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
            assert!(load(root.path()).is_err());
        }
    }
    #[test]
    fn negative_directory_ids_do_not_hide_reserved_kernel_uid_collisions() {
        assert_eq!(native_id("-2").unwrap(), 4294967294);
        assert_eq!(native_id("51001").unwrap(), 51001);
        assert!(native_id("4294967296").is_err());
    }
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn apply_off_macos_refuses_without_even_reading_input() {
        assert!(apply(Path::new("/does-not-exist"))
            .unwrap_err()
            .to_string()
            .contains("native Darwin"));
    }
}

#[cfg(test)]
mod membership_tests {
    #[test]
    fn ambiguous_nonmembership_errors_are_not_negative_proof() {
        assert!(super::not_admin(
            "no _stjobs_123abc is NOT a member of admin\n",
            "_stjobs_123abc"
        ));
        assert!(!super::not_admin("no admin group found", "_stjobs_123abc"));
        assert!(!super::not_admin(
            "yes _stjobs_123abc is a member of admin",
            "_stjobs_123abc"
        ));
    }
}
