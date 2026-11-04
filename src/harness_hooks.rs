//! The query-first hooks for Claude Code and Codex (aegis-wwl).
//!
//! A shantytown crew gets its query-first directive from the role settings st
//! launches it with. Anyone else who installs the stack runs plain `claude` or
//! `codex`, which read only the user's own settings, so until now they got no
//! query-first block at all. `tambo hooks install` adds two tambo-owned hooks
//! (SessionStart and UserPromptSubmit) to `~/.claude/settings.json` and
//! `~/.codex/config.toml`. Each one runs `tambo hook query-first`, which prints
//! the directive for the configured Quipu and appends one line to a local usage
//! counter.
//!
//! Install keeps every existing hook and is idempotent: a second run writes
//! nothing. Before the first change it saves the file's exact bytes (or that it
//! was absent). Uninstall puts those bytes back when the file still holds only
//! what install wrote. If the file was edited since then, uninstall removes only
//! the tambo entries and says so.

use std::{
    env, fs,
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::doctor::{Finding, Level};

/// The two events that carry the directive.
pub const EVENTS: [&str; 2] = ["SessionStart", "UserPromptSubmit"];
/// Every tambo-owned hook command contains this, and no other hook does.
const MARK: &str = " hook query-first --harness ";
const TIMEOUT_SECONDS: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Harness {
    Claude,
    Codex,
}

impl Harness {
    pub const ALL: [Harness; 2] = [Harness::Claude, Harness::Codex];

    pub fn name(self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::Codex => "codex",
        }
    }

    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "claude" => Ok(Harness::Claude),
            "codex" => Ok(Harness::Codex),
            other => bail!("unknown harness {other:?}; expected claude or codex"),
        }
    }
}

/// Where the harness settings and tambo's hook state live.
#[derive(Debug, Clone)]
pub struct Paths {
    pub claude_settings: PathBuf,
    pub codex_config: PathBuf,
    pub state: PathBuf,
}

impl Paths {
    /// `$CLAUDE_CONFIG_DIR` (default `~/.claude`), `$CODEX_HOME` (default
    /// `~/.codex`) and `$XDG_STATE_HOME/tambo/agent-hooks` (default
    /// `~/.local/state/...`), the same places the harnesses themselves read.
    pub fn from_env() -> Result<Self> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set")?;
        let dir = |var: &str, default: PathBuf| {
            env::var_os(var)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or(default)
        };
        Ok(Self {
            claude_settings: dir("CLAUDE_CONFIG_DIR", home.join(".claude")).join("settings.json"),
            codex_config: dir("CODEX_HOME", home.join(".codex")).join("config.toml"),
            state: dir("XDG_STATE_HOME", home.join(".local/state"))
                .join("tambo")
                .join("agent-hooks"),
        })
    }

    fn settings(&self, harness: Harness) -> &Path {
        match harness {
            Harness::Claude => &self.claude_settings,
            Harness::Codex => &self.codex_config,
        }
    }

    fn before(&self, harness: Harness) -> PathBuf {
        self.state.join(format!("{}.before", harness.name()))
    }

    fn absent_marker(&self, harness: Harness) -> PathBuf {
        self.state.join(format!("{}.before-absent", harness.name()))
    }

    fn after_digest(&self, harness: Harness) -> PathBuf {
        self.state.join(format!("{}.after.sha256", harness.name()))
    }

    fn config(&self) -> PathBuf {
        self.state.join("config.json")
    }

    pub fn usage(&self) -> PathBuf {
        self.state.join("usage.jsonl")
    }
}

/// Which Quipu the directive names. Saved at install so the hook needs no plan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Target {
    /// Absolute path of the plan's local store.
    pub db: Option<PathBuf>,
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

/// The exact hook command for one harness and event.
pub fn command(tambo: &Path, harness: Harness, event: &str) -> String {
    format!(
        "{}{MARK}{} --event {event}",
        quote(&tambo.display().to_string()),
        harness.name()
    )
}

fn is_ours(command: &str) -> bool {
    command.contains(MARK)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("read {}", path.display())),
    }
}

/// Replace `path` atomically, keeping an existing file's mode (new files 0600).
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("settings path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let mode = fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o7777)
        .unwrap_or(0o600);
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    fs::set_permissions(tmp.path(), fs::Permissions::from_mode(mode))?;
    tmp.persist(path)
        .with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

fn private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(path.parent().context("state path has no parent")?)?;
    let mut tmp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    tmp.write_all(bytes)?;
    fs::set_permissions(tmp.path(), fs::Permissions::from_mode(0o600))?;
    tmp.persist(path)?;
    Ok(())
}

// --- Claude Code: settings.json -----------------------------------------------

fn claude_parse(bytes: Option<&[u8]>) -> Result<Value> {
    match bytes {
        None => Ok(json!({})),
        Some(b) if b.iter().all(u8::is_ascii_whitespace) => Ok(json!({})),
        Some(b) => {
            let v: Value = serde_json::from_slice(b).context(
                "Claude settings.json is not valid JSON; fix it first, nothing was written",
            )?;
            if !v.is_object() {
                bail!("Claude settings.json is not a JSON object; nothing was written");
            }
            Ok(v)
        }
    }
}

fn claude_commands(settings: &Value, event: &str) -> Vec<String> {
    settings["hooks"][event]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|group| group["hooks"].as_array().into_iter().flatten())
        .filter_map(|h| h["command"].as_str().map(str::to_string))
        .collect()
}

fn claude_remove(settings: &mut Value) {
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    for event in EVENTS {
        let Some(groups) = hooks.get_mut(event).and_then(Value::as_array_mut) else {
            continue;
        };
        for group in groups.iter_mut() {
            if let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                list.retain(|h| !h["command"].as_str().is_some_and(is_ours));
            }
        }
        groups.retain(|g| !g["hooks"].as_array().is_some_and(Vec::is_empty));
        if groups.is_empty() {
            hooks.remove(event);
        }
    }
    if hooks.is_empty() {
        settings.as_object_mut().unwrap().remove("hooks");
    }
}

fn claude_render(current: Option<&[u8]>, wanted: &[(String, String)]) -> Result<Option<Vec<u8>>> {
    let mut settings = claude_parse(current)?;
    if wanted
        .iter()
        .all(|(event, cmd)| claude_commands(&settings, event).iter().any(|c| c == cmd))
        && EVENTS.iter().all(|e| {
            claude_commands(&settings, e)
                .iter()
                .filter(|c| is_ours(c))
                .count()
                == 1
        })
    {
        return Ok(None);
    }
    claude_remove(&mut settings);
    let object = settings.as_object_mut().unwrap();
    let hooks = object.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .context("Claude settings.json `hooks` is not an object; nothing was written")?;
    for (event, cmd) in wanted {
        let groups = hooks.entry(event.clone()).or_insert_with(|| json!([]));
        groups
            .as_array_mut()
            .with_context(|| format!("Claude settings.json hooks.{event} is not a list"))?
            .push(json!({"hooks":[{"type":"command","command":cmd,"timeout":TIMEOUT_SECONDS}]}));
    }
    let mut out = serde_json::to_vec_pretty(&settings)?;
    out.push(b'\n');
    Ok(Some(out))
}

fn claude_strip(current: &[u8]) -> Result<Vec<u8>> {
    let mut settings = claude_parse(Some(current))?;
    claude_remove(&mut settings);
    let mut out = serde_json::to_vec_pretty(&settings)?;
    out.push(b'\n');
    Ok(out)
}

// --- Codex: config.toml -------------------------------------------------------

fn codex_parse(bytes: Option<&[u8]>) -> Result<toml_edit::DocumentMut> {
    let text = match bytes {
        None => String::new(),
        Some(b) => String::from_utf8(b.to_vec()).context("Codex config.toml is not UTF-8")?,
    };
    text.parse::<toml_edit::DocumentMut>()
        .context("Codex config.toml is not valid TOML; fix it first, nothing was written")
}

fn codex_commands(doc: &toml_edit::DocumentMut, event: &str) -> Vec<String> {
    let Some(groups) = doc
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(toml_edit::Item::as_array_of_tables)
    else {
        return Vec::new();
    };
    groups
        .iter()
        .filter_map(|g| g.get("hooks").and_then(toml_edit::Item::as_array_of_tables))
        .flat_map(|list| list.iter())
        .filter_map(|h| {
            h.get("command")
                .and_then(|c| c.as_str())
                .map(str::to_string)
        })
        .collect()
}

fn snake(event: &str) -> &'static str {
    match event {
        "SessionStart" => "session_start",
        _ => "user_prompt_submit",
    }
}

/// Codex keys its hook-trust ledger (`[hooks.state]`) by
/// `<config path>:<event>:<group>:<hook>`. These are the keys of our hooks.
fn codex_trust_keys(doc: &toml_edit::DocumentMut, config: &Path) -> Vec<String> {
    let mut names = vec![config.display().to_string()];
    // Codex may record the path as given or resolved (macOS /var -> /private/var).
    if let Ok(real) = fs::canonicalize(config) {
        let real = real.display().to_string();
        if !names.contains(&real) {
            names.push(real);
        }
    }
    let mut keys = Vec::new();
    for event in EVENTS {
        let Some(groups) = doc
            .get("hooks")
            .and_then(|h| h.get(event))
            .and_then(toml_edit::Item::as_array_of_tables)
        else {
            continue;
        };
        for (g, group) in groups.iter().enumerate() {
            let Some(list) = group
                .get("hooks")
                .and_then(toml_edit::Item::as_array_of_tables)
            else {
                continue;
            };
            for (h, hook) in list.iter().enumerate() {
                if hook
                    .get("command")
                    .and_then(|c| c.as_str())
                    .is_some_and(is_ours)
                {
                    for name in &names {
                        keys.push(format!("{name}:{}:{g}:{h}", snake(event)));
                    }
                }
            }
        }
    }
    keys
}

/// Whether Codex has recorded trust for every one of our hooks.
fn codex_trusted(doc: &toml_edit::DocumentMut, config: &Path) -> bool {
    let state = doc.get("hooks").and_then(|h| h.get("state"));
    let keys = codex_trust_keys(doc, config);
    EVENTS.iter().all(|e| {
        keys.iter()
            .filter(|k| k.contains(&format!(":{}:", snake(e))))
            .any(|k| state.and_then(|s| s.get(k)).is_some())
    })
}

fn codex_remove(doc: &mut toml_edit::DocumentMut, config: &Path) {
    let trust = codex_trust_keys(doc, config);
    let Some(hooks) = doc
        .get_mut("hooks")
        .and_then(toml_edit::Item::as_table_like_mut)
    else {
        return;
    };
    if let Some(state) = hooks
        .get_mut("state")
        .and_then(toml_edit::Item::as_table_like_mut)
    {
        for key in &trust {
            state.remove(key);
        }
        if state.is_empty() {
            hooks.remove("state");
        }
    }
    for event in EVENTS {
        let Some(groups) = hooks
            .get_mut(event)
            .and_then(toml_edit::Item::as_array_of_tables_mut)
        else {
            continue;
        };
        for group in groups.iter_mut() {
            if let Some(list) = group
                .get_mut("hooks")
                .and_then(toml_edit::Item::as_array_of_tables_mut)
            {
                list.retain(|h| {
                    !h.get("command")
                        .and_then(|c| c.as_str())
                        .is_some_and(is_ours)
                });
            }
        }
        groups.retain(|g| {
            !g.get("hooks")
                .and_then(toml_edit::Item::as_array_of_tables)
                .is_some_and(|l| l.is_empty())
        });
        if groups.is_empty() {
            hooks.remove(event);
        }
    }
    if hooks.is_empty() {
        doc.remove("hooks");
    }
}

fn codex_render(
    current: Option<&[u8]>,
    wanted: &[(String, String)],
    config: &Path,
) -> Result<Option<Vec<u8>>> {
    let mut doc = codex_parse(current)?;
    if wanted
        .iter()
        .all(|(event, cmd)| codex_commands(&doc, event).iter().any(|c| c == cmd))
        && EVENTS.iter().all(|e| {
            codex_commands(&doc, e)
                .iter()
                .filter(|c| is_ours(c))
                .count()
                == 1
        })
    {
        return Ok(None);
    }
    codex_remove(&mut doc, config);
    if doc.get("hooks").is_none() {
        let mut table = toml_edit::Table::new();
        table.set_implicit(true);
        doc.insert("hooks", toml_edit::Item::Table(table));
    }
    let hooks = doc["hooks"]
        .as_table_mut()
        .context("Codex config.toml `hooks` is not a table; nothing was written")?;
    for (event, cmd) in wanted {
        let mut hook = toml_edit::Table::new();
        hook.insert("type", toml_edit::value("command"));
        hook.insert("command", toml_edit::value(cmd.as_str()));
        hook.insert("timeout", toml_edit::value(TIMEOUT_SECONDS));
        let mut list = toml_edit::ArrayOfTables::new();
        list.push(hook);
        let mut group = toml_edit::Table::new();
        group.insert("hooks", toml_edit::Item::ArrayOfTables(list));
        let entry = hooks.entry(event).or_insert(toml_edit::Item::ArrayOfTables(
            toml_edit::ArrayOfTables::new(),
        ));
        entry
            .as_array_of_tables_mut()
            .with_context(|| format!("Codex config.toml hooks.{event} is not an array of tables"))?
            .push(group);
    }
    Ok(Some(doc.to_string().into_bytes()))
}

fn codex_strip(current: &[u8], config: &Path) -> Result<Vec<u8>> {
    let mut doc = codex_parse(Some(current))?;
    codex_remove(&mut doc, config);
    Ok(doc.to_string().into_bytes())
}

fn render(
    harness: Harness,
    current: Option<&[u8]>,
    wanted: &[(String, String)],
    path: &Path,
) -> Result<Option<Vec<u8>>> {
    match harness {
        Harness::Claude => claude_render(current, wanted),
        Harness::Codex => codex_render(current, wanted, path),
    }
}

fn strip(harness: Harness, current: &[u8], path: &Path) -> Result<Vec<u8>> {
    match harness {
        Harness::Claude => claude_strip(current),
        Harness::Codex => codex_strip(current, path),
    }
}

fn configured(harness: Harness, current: Option<&[u8]>) -> Result<Vec<(String, String)>> {
    let mut found = Vec::new();
    match harness {
        Harness::Claude => {
            let settings = claude_parse(current)?;
            for e in EVENTS {
                for c in claude_commands(&settings, e)
                    .into_iter()
                    .filter(|c| is_ours(c))
                {
                    found.push((e.to_string(), c));
                }
            }
        }
        Harness::Codex => {
            let doc = codex_parse(current)?;
            for e in EVENTS {
                for c in codex_commands(&doc, e).into_iter().filter(|c| is_ours(c)) {
                    found.push((e.to_string(), c));
                }
            }
        }
    }
    Ok(found)
}

// --- install / uninstall / verify ---------------------------------------------

/// What happened to one harness's settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Written,
    Unchanged,
    Restored,
    Removed,
    NothingToRemove,
}

impl Outcome {
    pub fn describe(&self) -> &'static str {
        match self {
            Outcome::Written => "query-first hooks installed",
            Outcome::Unchanged => "query-first hooks already installed; nothing written",
            Outcome::Restored => "restored the exact settings saved before install",
            Outcome::Removed => {
                "removed the tambo hooks; the file changed after install, so other edits were kept"
            }
            Outcome::NothingToRemove => "no tambo hooks found; nothing written",
        }
    }
}

/// Add the query-first hooks for every harness, recording `target` for the hook.
pub fn install(paths: &Paths, tambo: &Path, target: &Target) -> Result<Vec<(Harness, Outcome)>> {
    if !tambo.is_absolute() {
        bail!(
            "the hook command needs tambo's absolute path, got {}",
            tambo.display()
        );
    }
    // Render every file first, so a malformed one stops the install before any write.
    let mut staged = Vec::new();
    for harness in Harness::ALL {
        let path = paths.settings(harness);
        let current = read_optional(path)?;
        let wanted: Vec<(String, String)> = EVENTS
            .iter()
            .map(|e| (e.to_string(), command(tambo, harness, e)))
            .collect();
        let next = render(harness, current.as_deref(), &wanted, path)?;
        staged.push((harness, current, next));
    }
    fs::create_dir_all(&paths.state)?;
    fs::set_permissions(&paths.state, fs::Permissions::from_mode(0o700))?;
    let config = json!({"schema":1,"db":target.db.as_ref().map(|d| d.display().to_string())});
    private_write(&paths.config(), &serde_json::to_vec_pretty(&config)?)?;
    let mut outcomes = Vec::new();
    for (harness, current, next) in staged {
        let Some(next) = next else {
            outcomes.push((harness, Outcome::Unchanged));
            continue;
        };
        // Only the first install saves the prior file: later runs must not
        // replace the user's original with a file that already holds our hooks.
        if !paths.before(harness).exists() && !paths.absent_marker(harness).exists() {
            match &current {
                Some(bytes) => private_write(&paths.before(harness), bytes)?,
                None => private_write(&paths.absent_marker(harness), b"")?,
            }
        }
        write_atomic(paths.settings(harness), &next)?;
        private_write(&paths.after_digest(harness), digest(&next).as_bytes())?;
        outcomes.push((harness, Outcome::Written));
    }
    Ok(outcomes)
}

/// Remove the hooks; restore the saved bytes when nothing else changed since.
pub fn uninstall(paths: &Paths) -> Result<Vec<(Harness, Outcome)>> {
    let mut outcomes = Vec::new();
    for harness in Harness::ALL {
        let path = paths.settings(harness);
        let current = read_optional(path)?;
        let after = read_optional(&paths.after_digest(harness))?
            .map(|b| String::from_utf8_lossy(&b).trim().to_string());
        let before = read_optional(&paths.before(harness))?;
        let was_absent = paths.absent_marker(harness).exists();
        let untouched = matches!((&current, &after), (Some(c), Some(a)) if digest(c) == *a);
        let outcome = match &current {
            Some(_) if untouched && was_absent => {
                fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
                Outcome::Restored
            }
            Some(_) if untouched && before.is_some() => {
                write_atomic(path, before.as_deref().unwrap())?;
                Outcome::Restored
            }
            Some(bytes) if !configured(harness, Some(bytes))?.is_empty() => {
                // Something else wrote the file since install (Codex records hook
                // trust and its own state in it). Remove only our hooks and their
                // trust entries; if that gives back the saved file, say so.
                let stripped = strip(harness, bytes, path)?;
                if before.as_deref() == Some(stripped.as_slice()) {
                    write_atomic(path, &stripped)?;
                    Outcome::Restored
                } else if was_absent && is_empty_settings(harness, &stripped)? {
                    fs::remove_file(path)?;
                    Outcome::Restored
                } else {
                    write_atomic(path, &stripped)?;
                    Outcome::Removed
                }
            }
            _ => Outcome::NothingToRemove,
        };
        for leftover in [
            paths.before(harness),
            paths.absent_marker(harness),
            paths.after_digest(harness),
        ] {
            if leftover.exists() {
                fs::remove_file(&leftover)?;
            }
        }
        outcomes.push((harness, outcome));
    }
    Ok(outcomes)
}

fn is_empty_settings(harness: Harness, bytes: &[u8]) -> Result<bool> {
    Ok(match harness {
        Harness::Claude => claude_parse(Some(bytes))?
            .as_object()
            .is_some_and(|o| o.is_empty()),
        Harness::Codex => codex_parse(Some(bytes))?.as_table().is_empty(),
    })
}

fn program(command: &str) -> Option<PathBuf> {
    let rest = command.strip_prefix('\'')?;
    let end = rest.find('\'')?;
    Some(PathBuf::from(&rest[..end]))
}

fn executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// One finding per harness: installed, missing or broken, with the fix.
pub fn verify(paths: &Paths) -> Vec<Finding> {
    let mut findings = Vec::new();
    for harness in Harness::ALL {
        let subject = format!("{} query-first hooks", harness.name());
        let path = paths.settings(harness);
        let found = match read_optional(path).and_then(|b| configured(harness, b.as_deref())) {
            Ok(found) => found,
            Err(e) => {
                findings.push(Finding {
                    level: Level::Fail,
                    subject,
                    detail: format!("cannot read {}: {e:#}", path.display()),
                });
                continue;
            }
        };
        let missing: Vec<&str> = EVENTS
            .iter()
            .copied()
            .filter(|e| !found.iter().any(|(event, _)| event == e))
            .collect();
        let broken: Vec<String> = found
            .iter()
            .filter(|(_, c)| !program(c).is_some_and(|p| executable(&p)))
            .map(|(e, c)| {
                format!(
                    "{e} runs {}",
                    program(c)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| c.clone())
                )
            })
            .collect();
        let duplicated = EVENTS
            .iter()
            .any(|e| found.iter().filter(|(event, _)| event == e).count() > 1);
        let (level, detail) = if !broken.is_empty() {
            (
                Level::Fail,
                format!(
                    "broken ({}, not executable); fix: tambo hooks install",
                    broken.join("; ")
                ),
            )
        } else if missing.len() == EVENTS.len() {
            (
                Level::Warn,
                format!(
                    "not installed in {}; fix: tambo hooks install",
                    path.display()
                ),
            )
        } else if !missing.is_empty() || duplicated {
            (
                Level::Fail,
                format!(
                    "incomplete or duplicated in {} (missing: {}); fix: tambo hooks install",
                    path.display(),
                    missing.join(", ")
                ),
            )
        } else if harness == Harness::Codex
            && !read_optional(path)
                .ok()
                .flatten()
                .and_then(|b| codex_parse(Some(&b)).ok())
                .is_some_and(|doc| codex_trusted(&doc, path))
        {
            (
                Level::Warn,
                "installed but not trusted yet, so Codex will not run them; fix: start `codex` and choose \"Trust all and continue\" (or \"Review hooks\" first)".to_string(),
            )
        } else {
            (
                Level::Ok,
                format!(
                    "installed in {}{}",
                    path.display(),
                    last_firing(paths, harness)
                ),
            )
        };
        findings.push(Finding {
            level,
            subject,
            detail,
        });
    }
    findings
}

fn last_firing(paths: &Paths, harness: Harness) -> String {
    let Ok(text) = fs::read_to_string(paths.usage()) else {
        return "; not fired yet".into();
    };
    let count = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["harness"] == harness.name())
        .count();
    if count == 0 {
        "; not fired yet".into()
    } else {
        format!("; fired {count} times")
    }
}

// --- the hook itself ----------------------------------------------------------

fn load_target(paths: &Paths) -> Target {
    let db = fs::read(paths.config())
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v["db"].as_str().map(PathBuf::from));
    Target { db }
}

/// The text a session sees. `None` means print nothing.
pub fn directive(
    event: &str,
    server: Option<&str>,
    target: &Target,
    crew: bool,
) -> (Option<String>, &'static str, Option<bool>) {
    if crew {
        // st launched this session with role settings that carry their own
        // query-first directive; a second copy is noise.
        return (None, "crew-managed", None);
    }
    let (mode, how, reachable) = match (server, &target.db) {
        (Some(url), _) => (
            "remote",
            format!(
                "Use the quipu MCP tool quipu_search {{\"query\":\"<subject>\"}} if it is registered, or:\n  curl -s {url}/search -H 'content-type: application/json' -d '{{\"query\":\"<subject>\"}}'"
            ),
            None,
        ),
        (None, Some(db)) => (
            "local",
            format!(
                "Run: quipu search \"<subject>\" --db {}",
                quote(&db.display().to_string())
            ),
            Some(db.exists()),
        ),
        (None, None) => (
            "unconfigured",
            "No Quipu store is configured for these hooks; run `tambo hooks install` from your tambo work directory.".to_string(),
            Some(false),
        ),
    };
    let text = if event == "SessionStart" {
        let status = match reachable {
            Some(false) if target.db.is_some() => format!(
                "\nUNAVAILABLE: the store {} does not exist, so nothing can be looked up. That is not the same as \"nothing is known\".",
                target.db.as_ref().unwrap().display()
            ),
            _ => String::new(),
        };
        format!(
            "QUERY-FIRST (knowledge graph): before you start a task, ask Quipu what is already known about what you are about to touch (earlier incidents, decisions, where facts came from) instead of re-deriving it.\n  {how}\nQuery first, grep second.{status}"
        )
    } else {
        format!(
            "Query-first reminder: check Quipu for what this prompt touches before acting. {how}"
        )
    };
    (Some(text), mode, reachable)
}

/// `tambo hook query-first`: print the directive and count the firing.
/// Always exits 0; a hook must never block a session.
pub fn run_hook(harness: Harness, event: &str) -> Result<()> {
    if !EVENTS.contains(&event) {
        bail!("unsupported query-first event {event:?}");
    }
    let mut input = Vec::new();
    let _ = std::io::stdin().take(1 << 20).read_to_end(&mut input);
    let session = serde_json::from_slice::<Value>(&input)
        .ok()
        .and_then(|v| v["session_id"].as_str().map(str::to_string));
    let paths = Paths::from_env()?;
    let target = load_target(&paths);
    let server = env::var("QUIPU_SERVER")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let crew = env::var_os("SHANTY_AGENT").is_some() && env::var_os("SHANTY_ROOT").is_some();
    let (text, mode, reachable) = directive(event, server.as_deref(), &target, crew);
    if let Some(text) = text {
        println!("{text}");
    }
    let at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let line = json!({"at":at,"harness":harness.name(),"event":event,"mode":mode,
        "reachable":reachable,"session":session,"printed":mode != "crew-managed"});
    if fs::create_dir_all(&paths.state).is_ok() {
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(paths.usage())
        {
            let _ = writeln!(f, "{line}");
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "harness_hooks_tests.rs"]
mod tests;
