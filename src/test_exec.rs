//! Test-only: write a fake executable that a parallel test cannot make busy.
//!
//! Parallel tests write scripts and then exec them. If any sibling thread forks
//! while this process holds the script open for writing, the forked child
//! inherits that descriptor until it calls exec (`O_CLOEXEC` does not close the
//! window between fork and exec). An exec of the script in that window fails
//! with ETXTBSY, "Text file busy" (aegis-zke).
//!
//! Writing through a short-lived `sh` child means the write descriptor only
//! ever exists in that child. This process never holds it, so no sibling fork
//! can inherit it, and once the child has exited the file has no writer.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Write `contents` to `path` with mode 0755, from a child process.
pub(crate) fn write_executable(path: &Path, contents: &str) {
    let mut child = Command::new("/bin/sh")
        .args(["-c", "cat > \"$1\" && chmod 0755 \"$1\"", "sh"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn sh to write a fake executable");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(contents.as_bytes())
        .expect("write fake executable contents");
    let status = child.wait().expect("wait for the fake executable writer");
    assert!(status.success(), "writing {} failed", path.display());
}
