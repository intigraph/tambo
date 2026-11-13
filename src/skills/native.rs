//! Native commands own share integrity, composition admission and atomic rendering.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn regular_bytes(path: &Path, max: u64) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.file_type().is_symlink() || before.len() > max {
        bail!(
            "skill input must be a bounded regular file: {}",
            path.display()
        );
    }
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let opened = file.metadata()?;
    let identity = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.mode(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if identity(&before) != identity(&opened) {
        bail!("skill input changed before read");
    }
    let mut bytes = Vec::new();
    (&mut file).take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max || identity(&opened) != identity(&file.metadata()?) {
        bail!("skill input changed during read or exceeds budget");
    }
    Ok(bytes)
}

pub(super) struct Native {
    program: PathBuf,
    pub db: PathBuf,
}
impl Native {
    pub fn new(program: &Path, db: PathBuf) -> Self {
        Self {
            program: program.into(),
            db,
        }
    }
    pub fn call(&self, args: &[&std::ffi::OsStr]) -> Result<String> {
        let out = Command::new(&self.program)
            .args(args)
            .arg("--db")
            .arg(&self.db)
            .current_dir(
                self.db
                    .parent()
                    .context("native store has no private parent")?,
            )
            .output()
            .context("start native Quipu skill operation")?;
        if !out.status.success() {
            bail!(
                "native skill operation refused: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        if out.stdout.len() > 4 * 1024 * 1024 {
            bail!("native skill response exceeds budget");
        }
        Ok(String::from_utf8(out.stdout)?)
    }
    pub fn json(&self, args: &[&std::ffi::OsStr]) -> Result<Value> {
        let value: Value = serde_json::from_str(&self.call(args)?)
            .context("native operation returned no JSON receipt")?;
        if value.get("conforms") == Some(&Value::Bool(false)) || value.get("error").is_some() {
            bail!("native skill receipt refused");
        }
        Ok(value)
    }
    pub fn spec(&self, iri: &str, spec: &Value, shape: &Path, file: &Path) -> Result<()> {
        let mut turtle = format!("<{iri}> a <urn:quipu:render:RenderSpec> .\n");
        for (key, value) in spec.as_object().context("render spec is not an object")? {
            let lexical = match value {
                Value::String(s) => serde_json::to_string(s)?,
                Value::Bool(b) => b.to_string(),
                _ => serde_json::to_string(&serde_json::to_string(value)?)?,
            };
            turtle.push_str(&format!("<{iri}> <urn:quipu:render:{key}> {lexical} .\n"));
        }
        fs::write(file, turtle)?;
        self.call(&[
            "knot".as_ref(),
            file.as_os_str(),
            "--shapes".as_ref(),
            shape.as_os_str(),
        ])?;
        Ok(())
    }
    pub fn render(&self, iri: &str, graph: Option<&str>, dest: &Path) -> Result<Value> {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "render".as_ref(),
            iri.as_ref(),
            "--dest".as_ref(),
            dest.as_os_str(),
        ];
        if let Some(g) = graph {
            args.push(std::ffi::OsStr::new("--graph"));
            args.push(std::ffi::OsStr::new(g));
        }
        let result = self.json(&args)?;
        if result
            .pointer("/diff/noop")
            .and_then(Value::as_bool)
            .is_none()
            || result.get("dry_run") == Some(&Value::Bool(true))
        {
            bail!("native render returned no applied/NOOP receipt; preserve destination for inspection");
        }
        Ok(result)
    }
}

pub(super) fn compile(source: &Path) -> Result<Value> {
    let output = Command::new("python3")
        .arg("-c")
        .arg(include_str!("../../scripts/skill-render-source.py"))
        .arg(source)
        .output()?;
    if !output.status.success() {
        bail!(
            "skill source refused: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub(super) fn single(content: &str, path: &str, includes: Vec<Value>) -> Value {
    json!({"query":format!("SELECT ?content WHERE {{ VALUES ?content {{ {} }} }}", serde_json::to_string(content).unwrap()),
        "layout":"single", "path":path, "template":"{{ rows[0].content }}", "provenance":false, "includes":includes})
}
