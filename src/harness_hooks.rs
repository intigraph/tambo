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

/// JSON that keeps the user's key order. serde_json's `Value` sorts object keys
/// (this crate does not enable `preserve_order`, and must not: other modules
/// hash serialized JSON), so editing through it would reorder the whole file.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum J {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<J>),
    Object(indexmap::IndexMap<String, J>),
}

impl J {
    fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Object(map) => map.get(key),
            _ => None,
        }
    }
    fn get_mut(&mut self, key: &str) -> Option<&mut J> {
        match self {
            J::Object(map) => map.get_mut(key),
            _ => None,
        }
    }
    fn array(&self) -> Option<&Vec<J>> {
        match self {
            J::Array(items) => Some(items),
            _ => None,
        }
    }
    fn array_mut(&mut self) -> Option<&mut Vec<J>> {
        match self {
            J::Array(items) => Some(items),
            _ => None,
        }
    }
    fn object_mut(&mut self) -> Option<&mut indexmap::IndexMap<String, J>> {
        match self {
            J::Object(map) => Some(map),
            _ => None,
        }
    }
    fn str(&self) -> Option<&str> {
        match self {
            J::String(s) => Some(s),
            _ => None,
        }
    }
}

fn claude_parse(bytes: Option<&[u8]>) -> Result<J> {
    let empty = || J::Object(indexmap::IndexMap::new());
    match bytes {
        None => Ok(empty()),
        Some(b) if b.iter().all(u8::is_ascii_whitespace) => Ok(empty()),
        Some(b) => {
            let v: J = serde_json::from_slice(b).context(
                "Claude settings.json is not valid JSON; fix it first, nothing was written",
            )?;
            if !matches!(v, J::Object(_)) {
                bail!("Claude settings.json is not a JSON object; nothing was written");
            }
            Ok(v)
        }
    }
}

fn claude_serialize(settings: &J) -> Result<Vec<u8>> {
    let mut out = serde_json::to_vec_pretty(settings)?;
    out.push(b'\n');
    Ok(out)
}

/// `(group, hook, command)` for every hook under one event.
fn claude_hooks(settings: &J, event: &str) -> Vec<(usize, usize, String)> {
    let mut found = Vec::new();
    let groups = settings
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(J::array);
    for (g, group) in groups.into_iter().flatten().enumerate() {
        for (h, hook) in group
            .get("hooks")
            .and_then(J::array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if let Some(cmd) = hook.get("command").and_then(J::str) {
                found.push((g, h, cmd.to_string()));
            }
        }
    }
    found
}

fn claude_commands(settings: &J, event: &str) -> Vec<String> {
    claude_hooks(settings, event)
        .into_iter()
        .map(|(_, _, c)| c)
        .collect()
}

/// Drop tambo hooks (all of them, or all but the first per event when
/// `keep_first`), then any group or event left empty by that.
fn claude_remove(settings: &mut J, keep_first: bool) {
    let Some(hooks) = settings.get_mut("hooks").and_then(J::object_mut) else {
        return;
    };
    for event in EVENTS {
        let Some(groups) = hooks.get_mut(event).and_then(J::array_mut) else {
            continue;
        };
        let mut kept = !keep_first;
        for group in groups.iter_mut() {
            if let Some(list) = group.get_mut("hooks").and_then(J::array_mut) {
                list.retain(|h| {
                    if !h.get("command").and_then(J::str).is_some_and(is_ours) {
                        return true;
                    }
                    if kept {
                        return false;
                    }
                    kept = true;
                    true
                });
            }
        }
        groups.retain(|g| !g.get("hooks").and_then(J::array).is_some_and(Vec::is_empty));
        if groups.is_empty() {
            hooks.shift_remove(event);
        }
    }
    if hooks.is_empty() {
        settings.object_mut().unwrap().shift_remove("hooks");
    }
}

fn claude_render(current: Option<&[u8]>, wanted: &[(String, String)]) -> Result<Option<Vec<u8>>> {
    let mut settings = claude_parse(current)?;
    let original = settings.clone();
    claude_remove(&mut settings, true);
    for (event, cmd) in wanted {
        let ours = claude_hooks(&settings, event)
            .into_iter()
            .find(|(_, _, c)| is_ours(c));
        if let Some((g, h, existing)) = ours {
            // Replace in place: the hook keeps its position among the user's.
            if existing != *cmd {
                let hook = &mut settings
                    .get_mut("hooks")
                    .unwrap()
                    .get_mut(event)
                    .unwrap()
                    .array_mut()
                    .unwrap()[g]
                    .get_mut("hooks")
                    .unwrap()
                    .array_mut()
                    .unwrap()[h];
                *hook.get_mut("command").unwrap() = J::String(cmd.clone());
            }
            continue;
        }
        let object = settings.object_mut().unwrap();
        let hooks = object
            .entry("hooks".to_string())
            .or_insert_with(|| J::Object(indexmap::IndexMap::new()))
            .object_mut()
            .context("Claude settings.json `hooks` is not an object; nothing was written")?;
        let group: J = serde_json::from_value(
            json!({"hooks":[{"type":"command","command":cmd,"timeout":TIMEOUT_SECONDS}]}),
        )?;
        hooks
            .entry(event.clone())
            .or_insert_with(|| J::Array(Vec::new()))
            .array_mut()
            .with_context(|| format!("Claude settings.json hooks.{event} is not a list"))?
            .push(group);
    }
    if settings == original {
        return Ok(None);
    }
    Ok(Some(claude_serialize(&settings)?))
}

fn claude_strip(current: &[u8]) -> Result<Vec<u8>> {
    let mut settings = claude_parse(Some(current))?;
    claude_remove(&mut settings, false);
    claude_serialize(&settings)
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

/// The names Codex may have recorded this config under: the path as given and,
/// when different, resolved (macOS /var -> /private/var).
fn trust_names(config: &Path) -> Vec<String> {
    let mut names = vec![config.display().to_string()];
    if let Ok(real) = fs::canonicalize(config) {
        let real = real.display().to_string();
        if !names.contains(&real) {
            names.push(real);
        }
    }
    names
}

/// `(group, hook, is_ours)` for every hook under one event, in file order.
/// Codex counts every hook entry, so positions here include the user's.
fn codex_positions(doc: &toml_edit::DocumentMut, event: &str) -> Vec<(usize, usize, bool)> {
    let mut found = Vec::new();
    let Some(groups) = doc
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(toml_edit::Item::as_array_of_tables)
    else {
        return found;
    };
    for (g, group) in groups.iter().enumerate() {
        let Some(list) = group
            .get("hooks")
            .and_then(toml_edit::Item::as_array_of_tables)
        else {
            continue;
        };
        for (h, hook) in list.iter().enumerate() {
            let ours = hook
                .get("command")
                .and_then(|c| c.as_str())
                .is_some_and(is_ours);
            found.push((g, h, ours));
        }
    }
    found
}

/// Codex keys its hook-trust ledger (`[hooks.state]`) by
/// `<config path>:<event>:<group>:<hook>`. These are the keys of our hooks.
fn codex_trust_keys(doc: &toml_edit::DocumentMut, config: &Path) -> Vec<String> {
    let names = trust_names(config);
    let mut keys = Vec::new();
    for event in EVENTS {
        for (g, h, ours) in codex_positions(doc, event) {
            if ours {
                for name in &names {
                    keys.push(format!("{name}:{}:{g}:{h}", snake(event)));
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

/// Drop tambo hooks (all, or all but the first per event when `keep_first`).
///
/// Codex's trust keys are positional, so removing a hook shifts every later
/// hook of that event. Each shifted hook's trust entry moves to its new key, so
/// the user's own hooks stay trusted; only the removed hooks' entries go.
fn codex_remove(doc: &mut toml_edit::DocumentMut, config: &Path, keep_first: bool) {
    let names = trust_names(config);
    let mut moves: Vec<(String, Option<String>)> = Vec::new(); // old key -> new key (None = drop)
    for event in EVENTS {
        let positions = codex_positions(doc, event);
        let mut kept_first = !keep_first;
        let keep: Vec<bool> = positions
            .iter()
            .map(|&(_, _, ours)| {
                if !ours {
                    return true;
                }
                if kept_first {
                    return false;
                }
                kept_first = true;
                true
            })
            .collect();
        // A group is dropped only when it had hooks and all of them go (the same
        // rule as the retain below); its new index counts the groups kept before it.
        let groups = doc
            .get("hooks")
            .and_then(|h| h.get(event))
            .and_then(toml_edit::Item::as_array_of_tables)
            .map_or(0, toml_edit::ArrayOfTables::len);
        let mut new_group = Vec::with_capacity(groups);
        let mut next = 0;
        for g in 0..groups {
            let mine: Vec<bool> = positions
                .iter()
                .zip(&keep)
                .filter(|(p, _)| p.0 == g)
                .map(|(_, k)| *k)
                .collect();
            if !mine.is_empty() && mine.iter().all(|k| !k) {
                new_group.push(None);
            } else {
                new_group.push(Some(next));
                next += 1;
            }
        }
        let mut next_hook: std::collections::HashMap<usize, usize> = Default::default();
        for (&(g, h, _), &k) in positions.iter().zip(&keep) {
            let new = if k {
                let ng = new_group[g].expect("a group with a kept hook is kept");
                let slot = next_hook.entry(ng).or_insert(0);
                let nh = *slot;
                *slot += 1;
                Some((ng, nh))
            } else {
                None
            };
            if new == Some((g, h)) {
                continue;
            }
            for name in &names {
                let old = format!("{name}:{}:{g}:{h}", snake(event));
                let target = new.map(|(ng, nh)| format!("{name}:{}:{ng}:{nh}", snake(event)));
                moves.push((old, target));
            }
        }
    }
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
        // Take every affected entry out first, so a move never lands on a key
        // that another move is about to vacate.
        let mut taken = Vec::new();
        for (old, target) in &moves {
            if let Some(item) = state.remove(old) {
                if let Some(target) = target {
                    taken.push((target.clone(), item));
                }
            }
        }
        for (target, item) in taken {
            state.insert(&target, item);
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
        let mut kept_first = !keep_first;
        for group in groups.iter_mut() {
            if let Some(list) = group
                .get_mut("hooks")
                .and_then(toml_edit::Item::as_array_of_tables_mut)
            {
                list.retain(|h| {
                    if !h
                        .get("command")
                        .and_then(|c| c.as_str())
                        .is_some_and(is_ours)
                    {
                        return true;
                    }
                    if kept_first {
                        return false;
                    }
                    kept_first = true;
                    true
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
    let original = doc.to_string();
    codex_remove(&mut doc, config, true);
    for (event, cmd) in wanted {
        let ours = codex_positions(&doc, event).into_iter().find(|p| p.2);
        if let Some((g, h, _)) = ours {
            // Replace in place, so no other hook's position (or trust key) moves.
            let hook = &mut doc["hooks"][event.as_str()]
                .as_array_of_tables_mut()
                .unwrap()
                .get_mut(g)
                .unwrap()["hooks"]
                .as_array_of_tables_mut()
                .unwrap()
                .get_mut(h)
                .unwrap();
            if hook.get("command").and_then(|c| c.as_str()) != Some(cmd.as_str()) {
                hook.insert("command", toml_edit::value(cmd.as_str()));
            }
            continue;
        }
        if doc.get("hooks").is_none() {
            let mut table = toml_edit::Table::new();
            table.set_implicit(true);
            doc.insert("hooks", toml_edit::Item::Table(table));
        }
        let hooks = doc["hooks"]
            .as_table_mut()
            .context("Codex config.toml `hooks` is not a table; nothing was written")?;
        let mut hook = toml_edit::Table::new();
        hook.insert("type", toml_edit::value("command"));
        hook.insert("command", toml_edit::value(cmd.as_str()));
        hook.insert("timeout", toml_edit::value(TIMEOUT_SECONDS));
        let mut list = toml_edit::ArrayOfTables::new();
        list.push(hook);
        let mut group = toml_edit::Table::new();
        group.insert("hooks", toml_edit::Item::ArrayOfTables(list));
        // Appended last, so no existing hook's position changes.
        hooks
            .entry(event)
            .or_insert(toml_edit::Item::ArrayOfTables(
                toml_edit::ArrayOfTables::new(),
            ))
            .as_array_of_tables_mut()
            .with_context(|| format!("Codex config.toml hooks.{event} is not an array of tables"))?
            .push(group);
    }
    let rendered = doc.to_string();
    if rendered == original {
        return Ok(None);
    }
    Ok(Some(rendered.into_bytes()))
}

fn codex_strip(current: &[u8], config: &Path) -> Result<Vec<u8>> {
    let mut doc = codex_parse(Some(current))?;
    codex_remove(&mut doc, config, false);
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

/// Why this host must not get user-level hooks, if it is a shantytown crew host.
///
/// Crew sessions are launched with st's role settings, which already carry the
/// query-first directive. Claude Code merges those with `~/.claude/settings.json`,
/// so user-level hooks would put a second copy in every crew session.
pub fn crew_host_reason(shanty_root: Option<&std::ffi::OsStr>, plan_crew: bool) -> Option<String> {
    if shanty_root.is_some_and(|r| !r.is_empty()) {
        return Some("SHANTY_ROOT is set".into());
    }
    plan_crew.then(|| "the plan selects a shantytown crew".into())
}

/// A settings file that is a symlink (a dotfile manager, say) would silently
/// become a regular file when replaced, so refuse instead.
fn refuse_symlink(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!(
            "{} is a symlink; tambo will not replace it with a regular file, and nothing was written. \
             Add the hooks to the file it points to, or replace the link with a regular file and rerun",
            path.display()
        );
    }
    Ok(())
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
        refuse_symlink(path)?;
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
    for harness in Harness::ALL {
        refuse_symlink(paths.settings(harness))?;
    }
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
        Harness::Claude => matches!(claude_parse(Some(bytes))?, J::Object(m) if m.is_empty()),
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
