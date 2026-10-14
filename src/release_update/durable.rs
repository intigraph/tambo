//! File and directory barriers shared by release swaps and their journal.
use super::*;

pub(super) fn checkpoint(_stage: &'static str, _path: &Path) -> Result<()> {
    #[cfg(test)]
    probe::record(_stage, _path)?;
    Ok(())
}

pub(super) fn sync_dir(path: &Path) -> Result<()> {
    checkpoint("directory-sync", path)?;
    fs::File::open(path)?.sync_all()?;
    checkpoint("directory-synced", path)
}

pub(super) fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

/// Publish every newly-created directory entry before relying on files below it.
pub(super) fn create_dirs(path: &Path) -> Result<()> {
    let ancestor = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty() && *p != path);
    if let Some(ancestor) = ancestor {
        create_dirs(ancestor)?;
    }
    match fs::metadata(path) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => bail!("release directory is not a directory: {}", path.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            match fs::create_dir(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir() => {}
                Err(e) => return Err(e.into()),
            }
            checkpoint("directory-created", path)?;
        }
        Err(e) => return Err(e.into()),
    }
    // Existing entries can be residue from a previous failed barrier. Retrying
    // must not assume those ancestors were durable merely because they exist.
    sync_dir(path)?;
    if let Some(ancestor) = ancestor {
        sync_dir(ancestor)?;
    }
    Ok(())
}

pub(super) fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let directory = parent(path);
    create_dirs(directory)?;
    let file = tempfile::NamedTempFile::new_in(directory)?;
    fs::write(file.path(), bytes)?;
    file.as_file().sync_all()?;
    checkpoint("file-synced", path)?;
    file.persist(path).map_err(|e| e.error)?;
    checkpoint("file-renamed", path)?;
    sync_dir(directory)
}

#[cfg(test)]
pub(super) mod probe {
    use super::*;
    use std::cell::RefCell;
    #[derive(Default)]
    pub struct Trace {
        pub events: Vec<(&'static str, PathBuf)>,
        pub fail: Option<(&'static str, PathBuf)>,
    }
    thread_local! { static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) }; }
    pub fn start(fail: Option<(&'static str, PathBuf)>) {
        TRACE.with(|t| {
            *t.borrow_mut() = Some(Trace {
                fail,
                ..Trace::default()
            })
        });
    }
    pub fn take() -> Trace {
        TRACE.with(|t| t.borrow_mut().take().unwrap())
    }
    pub fn record(stage: &'static str, path: &Path) -> Result<()> {
        TRACE.with(|t| {
            let mut trace = t.borrow_mut();
            if let Some(trace) = trace.as_mut() {
                trace.events.push((stage, path.to_path_buf()));
                if trace
                    .fail
                    .as_ref()
                    .is_some_and(|(s, p)| *s == stage && p == path)
                {
                    trace.fail = None; // One ordered fault; recovery barriers still execute.
                    bail!("injected release durability fault at {stage}");
                }
            }
            Ok(())
        })
    }
}
