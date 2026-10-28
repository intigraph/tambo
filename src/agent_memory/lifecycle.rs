use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use super::{render, BUNDLE_NAME};

pub trait Runner {
    fn run(&mut self, program: &str, args: &[String]) -> Result<String>;
    fn input(&mut self, _program: &str, _args: &[String], _body: &str) -> Result<String> {
        bail!("runner cannot exchange stdio metadata");
    }
    fn report(&mut self, program: &str, args: &[String]) -> Result<String> {
        self.run(program, args)
    }
}
pub struct System;
impl Runner for System {
    fn run(&mut self, program: &str, args: &[String]) -> Result<String> {
        let out = Command::new(program)
            .args(args)
            .output()
            .with_context(|| format!("start {program}"))?;
        if !out.status.success() {
            bail!(
                "{program} refused or failed (exit {:?}); no success claimed",
                out.status.code()
            );
        }
        String::from_utf8(out.stdout).context("command returned non-UTF8 output")
    }
    fn input(&mut self, program: &str, args: &[String], body: &str) -> Result<String> {
        use std::{
            io::Read,
            process::Stdio,
            time::{Duration, Instant},
        };
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        if let Err(error) = child
            .stdin
            .take()
            .context("no metadata stdin")?
            .write_all(body.as_bytes())
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
        let stdout = child.stdout.take().context("no metadata stdout")?;
        let reader = std::thread::spawn(move || {
            let mut out = Vec::new();
            stdout
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut out)
                .map(|_| out)
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    bail!("stdio metadata probe failed or exceeded deadline");
                }
            }
        };
        let out = reader
            .join()
            .map_err(|_| anyhow::anyhow!("metadata reader failed"))??;
        if !status.success() || out.len() > 4 * 1024 * 1024 {
            bail!("stdio metadata probe refused or exceeded output bound");
        }
        String::from_utf8(out).context("metadata response is not UTF8")
    }
    fn report(&mut self, program: &str, args: &[String]) -> Result<String> {
        let out = Command::new(program)
            .args(args)
            .output()
            .context("read owner report")?;
        if !matches!(out.status.code(), Some(0..=2)) {
            bail!("owner check could not run");
        }
        String::from_utf8(out.stdout).context("owner returned non-UTF8 report")
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub path: PathBuf,
    pub before: Option<String>,
    pub after: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: u32,
    pub root: PathBuf,
    pub source_revision: String,
    pub changes: Vec<Change>,
    pub bundles: Vec<Value>,
    pub expected_tools: Value,
    pub id: String,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn plan_id(p: &Plan) -> Result<String> {
    let mut clone = p.clone();
    clone.id.clear();
    Ok(digest(&serde_json::to_vec(&clone)?))
}
fn read(path: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(path) {
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => bail!(
            "configuration destination must be a regular file: {}",
            path.display()
        ),
        Ok(m) if m.len() > 4 * 1024 * 1024 => bail!("configuration exceeds 4MiB bound"),
        Ok(_) => Ok(Some(
            fs::read_to_string(path).context("read configuration")?,
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context("inspect configuration destination"),
    }
}
fn absolute(path: &Path) -> Result<PathBuf> {
    let full = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    if full
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        bail!("paths must not contain parent traversals");
    }
    if fs::symlink_metadata(&full).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("refusing symlink destination");
    }
    let mut ancestor = full.clone();
    let mut tail = Vec::new();
    while fs::symlink_metadata(&ancestor).is_err() {
        tail.push(
            ancestor
                .file_name()
                .context("path has no existing ancestor")?
                .to_os_string(),
        );
        ancestor = ancestor
            .parent()
            .context("path has no existing ancestor")?
            .to_path_buf();
    }
    let mut canonical = ancestor.canonicalize()?;
    for part in tail.into_iter().rev() {
        canonical.push(part);
    }
    Ok(canonical)
}
pub fn private_write(path: &Path, body: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let parent = path.parent().context("file has no parent")?;
    fs::create_dir_all(parent)?;
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("refusing symlink destination");
    }
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    let mode = fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o777)
        .unwrap_or(0o600);
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(mode))?;
    file.write_all(body)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

pub struct Create<'a> {
    pub root: &'a Path,
    pub bobbin: &'a Path,
    pub person: &'a str,
    pub output: &'a Path,
    pub source: &'a Path,
    pub claude_home: &'a Path,
    pub codex_home: &'a Path,
    pub issuer_name: Option<&'a str>,
}
pub fn create(options: Create<'_>, runner: &mut impl Runner) -> Result<()> {
    let Create {
        root,
        bobbin,
        person,
        output,
        source,
        claude_home,
        codex_home,
        issuer_name,
    } = options;
    use std::os::unix::fs::DirBuilderExt;
    let output = absolute(output)?;
    if output.exists() {
        bail!("plan output already exists; retain it for review/resume");
    }
    let root = absolute(root)?;
    let mut changes = Vec::new();
    let crew_path = root.join("shantytown.toml");
    let bobbin = absolute(bobbin)?;
    let crew_before = read(&crew_path)?;
    let bobbin_before = read(&bobbin)?;
    let (mut crew, config) = render(
        crew_before.as_deref().unwrap_or(""),
        bobbin_before.as_deref().unwrap_or(""),
        person,
    )?;
    let capture_path = output.join("capture.json");
    let mut document: toml_edit::DocumentMut = crew.parse()?;
    document["env"]["CAMAYOC_CAPTURE_CONFIG"] =
        toml_edit::value(capture_path.display().to_string());
    crew = document.to_string();
    let mut issuers = serde_json::Map::new();
    if let Some(name) = issuer_name {
        if name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
            bail!("invalid issuer name");
        }
        issuers.insert(name.to_string(), json!(person));
    }
    changes.push(Change {
        path: capture_path,
        before: None,
        after: serde_json::to_string_pretty(&json!({"prompt_author":person,"issuers":issuers}))?,
    });
    changes.push(Change {
        path: crew_path,
        before: crew_before,
        after: crew,
    });
    changes.push(Change {
        path: bobbin,
        before: bobbin_before,
        after: config,
    });
    // Read producer-generated surfaces from one exact immutable Git tree, not
    // from mutable working-tree files or a separately maintained Tambo template.
    let source = absolute(source)?;
    let revision = runner
        .run(
            "git",
            &[
                "-C".into(),
                source.display().to_string(),
                "rev-parse".into(),
                "HEAD".into(),
            ],
        )?
        .trim()
        .to_string();
    if revision.len() != 40 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("source has no full Git revision");
    }
    for (relative, destination) in [
        (
            "skills/camayoc-memory/SKILL.md",
            claude_home.join("skills/camayoc-memory/SKILL.md"),
        ),
        (
            ".agents/skills/camayoc-memory/SKILL.md",
            codex_home.join("skills/camayoc-memory/SKILL.md"),
        ),
    ] {
        let after = runner.run(
            "git",
            &[
                "-C".into(),
                source.display().to_string(),
                "show".into(),
                format!("{revision}:{relative}"),
            ],
        )?;
        if after.len() > 1024 * 1024 || !after.contains("cy record") {
            bail!("source does not contain the generated memory skill");
        }
        let path = absolute(&destination)?;
        changes.push(Change {
            before: read(&path)?,
            path,
            after,
        });
    }
    let catalogue = runner.run(
        "git",
        &[
            "-C".into(),
            source.display().to_string(),
            "show".into(),
            format!("{revision}:camayoc_cli/data/mcp-tools.json"),
        ],
    )?;
    if catalogue.len() > 4 * 1024 * 1024 {
        bail!("producer tool catalogue exceeds bound");
    }
    let catalogue: Value = serde_json::from_str(&catalogue)?;
    if catalogue["schema"] != "camayoc-mcp-tools/1"
        || catalogue["server"] != "camayoc-memory"
        || !catalogue["tools"].as_array().is_some_and(|t| !t.is_empty())
    {
        bail!("unsupported producer catalogue");
    }
    let mut paths = std::collections::BTreeSet::new();
    for change in &changes {
        if !paths.insert(&change.path) {
            bail!("memory destinations overlap");
        }
    }
    let candidate = runner
        .run("bobbin", &["hooks".into(), "bundle".into()])
        .ok();
    let bobbin_bundle =
        crate::tool_bundles::resolve_with(&crate::hook_bundles::selected(["bobbin"]), |_| {
            candidate.clone()
        });
    let mut plan = Plan {
        schema: 1,
        root,
        source_revision: revision,
        changes,
        expected_tools: catalogue["tools"].clone(),
        bundles: vec![
            serde_json::from_str(&bobbin_bundle[0].1)?,
            super::hooks::bundle(&output),
        ],
        id: String::new(),
    };
    plan.id = plan_id(&plan)?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&output)?;
    private_write(
        &output.join("plan.json"),
        &serde_json::to_vec_pretty(&plan)?,
    )?;
    for (i, bundle) in plan.bundles.iter().enumerate() {
        private_write(
            &output.join(format!("bundle-{i}.json")),
            &serde_json::to_vec_pretty(bundle)?,
        )?;
    }
    Ok(())
}

pub fn load(directory: &Path) -> Result<Plan> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let path = directory.join("plan.json");
    let meta = fs::symlink_metadata(&path)?;
    let dir = fs::symlink_metadata(directory)?;
    if meta.file_type().is_symlink()
        || !meta.is_file()
        || meta.len() > 16 * 1024 * 1024
        || meta.permissions().mode() & 0o077 != 0
        || dir.file_type().is_symlink()
        || !dir.is_dir()
        || dir.permissions().mode() & 0o077 != 0
        || dir.uid() != meta.uid()
    {
        bail!("memory plan must be private regular data in its private directory");
    }
    let plan: Plan = serde_json::from_slice(&fs::read(path)?)?;
    if plan.schema != 1 || plan.id != plan_id(&plan)? {
        bail!("memory plan digest/schema does not match; regenerate for review");
    }
    Ok(plan)
}
fn st(plan: &Plan, args: &[&str], runner: &mut impl Runner) -> Result<String> {
    let mut argv = vec!["--root".into(), plan.root.display().to_string()];
    argv.extend(args.iter().map(|s| (*s).to_string()));
    runner.run("st", &argv)
}
fn existing(plan: &Plan, runner: &mut impl Runner) -> Result<Vec<Value>> {
    let v: Value = serde_json::from_str(&st(plan, &["ops", "hooks", "list", "--json"], runner)?)?;
    if !v["errors"].as_array().is_some_and(Vec::is_empty) {
        bail!("settings-owner registry unreadable");
    }
    Ok(v["bundles"]
        .as_array()
        .context("owner returned no bundles")?
        .clone())
}
fn normalized(bundle: &Value) -> Value {
    json!({"name":bundle["name"],"version":bundle["version"],"owner":bundle["owner"],"roles":bundle["roles"],
        "hooks":bundle["hooks"].as_array().map(|rows| rows.iter().map(|r| json!({
            "event":r["event"],"command":r["command"],"matcher":r["matcher"],"timeout":r["timeout"],"harnesses":r.get("harnesses").cloned().unwrap_or_else(|| json!(["claude","codex","opencode"]))})).collect::<Vec<_>>())})
}
fn journal(directory: &Path, phase: &str) -> Result<()> {
    private_write(
        &directory.join("state.json"),
        &serde_json::to_vec(&json!({"schema":1,"phase":phase,
        "at":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs_f64()}))?,
    )
}
fn lock(directory: &Path) -> Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = directory.join("apply.lock");
    if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink() || !m.is_file()) {
        bail!("invalid memory lifecycle lock");
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)?;
    file.try_lock_exclusive()
        .context("another memory lifecycle operation is running")?;
    Ok(file)
}
pub fn check_tools(plan: &Plan, runner: &mut impl Runner) -> Result<()> {
    let requests=concat!("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"tambo-memory-verify\",\"version\":\"1\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n");
    let response = runner.input("cy", &["mcp".into()], requests)?;
    let rows: Vec<Value> = response
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let init = rows
        .iter()
        .find(|r| r["id"] == 1)
        .context("MCP initialization response missing")?;
    let list = rows
        .iter()
        .find(|r| r["id"] == 2)
        .context("MCP tools response missing")?;
    if init["result"]["serverInfo"]["name"] != "camayoc-memory"
        || list["result"]["tools"] != plan.expected_tools
    {
        bail!("installed Camayoc tool schemas differ from reviewed producer revision");
    }
    Ok(())
}
fn exact_bundles(plan: &Plan, directory: &Path, current: &[Value], prior: &[Value]) -> Result<()> {
    for (i, wanted) in plan.bundles.iter().enumerate() {
        let name = wanted["name"].as_str().context("bundle name missing")?;
        let old = prior.iter().find(|r| r["name"] == name);
        if let Some(old) = old {
            let backup: Value = serde_json::from_str(
                &read(&directory.join(format!("prior-{i}.json")))?
                    .context("prior bundle backup missing")?,
            )?;
            if &backup != old {
                bail!("prior bundle backup changed; preserve it for inspection");
            }
        }
        if current.iter().any(|r| r["name"] == name) {
            let raw: Value = serde_json::from_str(
                &read(&plan.root.join("hook-bundles").join(format!("{name}.json")))?
                    .context("owner bundle bytes unavailable")?,
            )?;
            if raw != *wanted && old != Some(&raw) {
                bail!("registered bundle bytes changed; preserve operator edits");
            }
        }
    }
    Ok(())
}
pub fn apply(directory: &Path, runner: &mut impl Runner) -> Result<()> {
    let plan = load(directory)?;
    let _lock = lock(directory)?;
    let saved = directory.join("registry-before.json");
    let current = existing(&plan, runner)?;
    if !saved.exists() {
        if current.iter().any(|r| r["name"] == BUNDLE_NAME) {
            bail!("memory bundle already owned by another plan; restore it first");
        }
        let mut snapshot = current.clone();
        for (i, wanted) in plan.bundles.iter().enumerate() {
            if let Some(prior) = current.iter().find(|r| r["name"] == wanted["name"]) {
                let name = prior["name"]
                    .as_str()
                    .context("owner bundle name missing")?;
                if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                    bail!("invalid bundle name");
                }
                let raw = read(&plan.root.join("hook-bundles").join(format!("{name}.json")))?
                    .context("exact prior bundle unavailable; no lossless rollback")?;
                let value: Value = serde_json::from_str(&raw)?;
                if normalized(&value) != normalized(prior) {
                    bail!("owner registry changed during snapshot");
                }
                if let Some(slot) = snapshot.iter_mut().find(|r| r["name"] == wanted["name"]) {
                    *slot = value;
                }
                private_write(&directory.join(format!("prior-{i}.json")), raw.as_bytes())?;
            }
        }
        private_write(&saved, &serde_json::to_vec(&snapshot)?)?;
    }
    let prior: Vec<Value> = serde_json::from_slice(&fs::read(&saved)?)?;
    for (i, wanted) in plan.bundles.iter().enumerate() {
        let path = directory.join(format!("bundle-{i}.json"));
        if serde_json::from_slice::<Value>(&fs::read(&path)?)? != *wanted {
            bail!("staged bundle differs from reviewed plan");
        }
        if let Some(now) = current.iter().find(|r| r["name"] == wanted["name"]) {
            if normalized(now) != normalized(wanted)
                && prior
                    .iter()
                    .find(|r| r["name"] == wanted["name"])
                    .map(normalized)
                    .as_ref()
                    != Some(&normalized(now))
            {
                bail!("registered bundle changed since plan application");
            }
        }
    }
    exact_bundles(&plan, directory, &current, &prior)?;
    // Every target is checked before the first write, including on resume.
    for c in &plan.changes {
        let now = read(&c.path)?;
        if now != c.before && now.as_deref() != Some(c.after.as_str()) {
            bail!(
                "configuration changed since plan review: {}",
                c.path.display()
            );
        }
    }
    for args in [
        vec!["--version"],
        vec!["record", "decision", "--help"],
        vec!["record", "directive", "--help"],
        vec!["record", "preference", "--help"],
        vec!["record", "handoff", "--help"],
        vec!["record", "reference", "--help"],
        vec!["hook", "memory-redirect", "--help"],
        vec!["flush", "--help"],
    ] {
        runner.run("cy", &args.iter().map(|s| (*s).to_string()).collect::<Vec<_>>())
            .context("install a current Camayoc CLI with its generated memory tools before applying this plan")?;
    }
    let help = runner.run("cy", &["flush".into(), "--help".into()])?;
    if !help.contains("--hook") {
        bail!("Camayoc lacks bounded hook replay; update it before applying");
    }
    runner.run(
        "cy",
        &["hook".into(), "session-end".into(), "--help".into()],
    )?;
    runner.run(
        "bobbin",
        &["hook".into(), "memory-refresh".into(), "--help".into()],
    )?;
    check_tools(&plan, runner)?;
    journal(directory, "applying")?;
    for c in &plan.changes {
        if read(&c.path)?.as_deref() != Some(c.after.as_str()) {
            private_write(&c.path, c.after.as_bytes())?;
        }
    }
    for (i, bundle) in plan.bundles.iter().enumerate() {
        let path = directory.join(format!("bundle-{i}.json"));
        let _ = bundle;
        st(
            &plan,
            &[
                "ops",
                "hooks",
                "register",
                path.to_str().context("non-UTF8 plan path")?,
            ],
            runner,
        )?;
    }
    let receipt: Value =
        serde_json::from_str(&st(&plan, &["ops", "provision", "--json"], runner)?)?;
    if receipt["version"] != 1
        || receipt["owner"] != "shantytown"
        || !receipt["agents"].as_array().is_some_and(|a| !a.is_empty())
    {
        bail!("settings owner returned no verified nonempty registration receipt");
    }
    for c in &plan.changes {
        if read(&c.path)?.as_deref() != Some(c.after.as_str()) {
            bail!("configuration read-back differs");
        }
    }
    for wanted in &plan.bundles {
        let name = wanted["name"].as_str().context("bundle has no name")?;
        let raw = read(&plan.root.join("hook-bundles").join(format!("{name}.json")))?
            .context("registered bundle missing on read-back")?;
        if serde_json::from_str::<Value>(&raw)? != *wanted {
            bail!("registered bundle read-back differs");
        }
    }
    journal(directory, "applied")
}

pub fn restore(directory: &Path, runner: &mut impl Runner) -> Result<()> {
    let plan = load(directory)?;
    let _lock = lock(directory)?;
    let saved: Vec<Value> = serde_json::from_slice(
        &fs::read(directory.join("registry-before.json")).context("plan was never applied")?,
    )?;
    let current = existing(&plan, runner)?;
    for c in &plan.changes {
        let now = read(&c.path)?;
        if now != c.before && now.as_deref() != Some(c.after.as_str()) {
            bail!(
                "preserve edited configuration; restore refused: {}",
                c.path.display()
            );
        }
    }
    for wanted in &plan.bundles {
        if let Some(now) = current.iter().find(|r| r["name"] == wanted["name"]) {
            let prior = saved.iter().find(|r| r["name"] == wanted["name"]);
            if normalized(now) != normalized(wanted)
                && prior.map(normalized).as_ref() != Some(&normalized(now))
            {
                bail!("registered bundle changed; restore refused");
            }
        }
    }
    exact_bundles(&plan, directory, &current, &saved)?;
    journal(directory, "restoring")?;
    for (i, wanted) in plan.bundles.iter().enumerate() {
        let name = wanted["name"].as_str().context("bundle has no name")?;
        if let Some(old) = saved.iter().find(|r| r["name"] == name) {
            // list receipts omit evidence; restoring from them would lose it.
            // The exact original bundle is saved separately before registration.
            let path = directory.join(format!("prior-{i}.json"));
            let _ = old;
            if !path.exists() {
                bail!("exact prior bundle required for lossless restore");
            }
            st(
                &plan,
                &[
                    "ops",
                    "hooks",
                    "register",
                    path.to_str().context("non-UTF8 path")?,
                ],
                runner,
            )?;
        } else if current.iter().any(|r| r["name"] == name) {
            st(&plan, &["ops", "hooks", "unregister", name], runner)?;
        }
    }
    for c in &plan.changes {
        if read(&c.path)? == c.before {
            continue;
        }
        if let Some(before) = &c.before {
            private_write(&c.path, before.as_bytes())?;
        } else if c.path.exists() {
            fs::remove_file(&c.path)?;
        }
    }
    st(&plan, &["ops", "provision", "--json"], runner)?;
    journal(directory, "restored")
}
