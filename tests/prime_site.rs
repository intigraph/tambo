//! Native curl and the compiled CLI against private loopback HTTP only.
#![cfg(unix)]
use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::Command;
use caboodle::prime::site;
use serde_json::{json, Value};

fn curl_path() -> OsString {
    let path = std::env::var_os("PATH");
    let curl = caboodle::adapter::which_all("curl", path.as_deref())
        .into_iter()
        .next()
        .expect("native curl is required by Tambo");
    curl.parent().unwrap().as_os_str().to_owned()
}

// Resolve the native curl in place: copying a platform binary can break its
// loader/signature assumptions on macOS. This read-only transport reference is
// never rewritten or stubbed; no managed stack program is placed on PATH.
fn private_curl() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let path = std::env::var_os("PATH");
    let curl = caboodle::adapter::which_all("curl", path.as_deref())
        .into_iter()
        .next()
        .unwrap();
    std::os::unix::fs::symlink(curl.canonicalize().unwrap(), dir.path().join("curl")).unwrap();
    dir
}

struct Server {
    url: String,
    request: Arc<Mutex<String>>,
    done: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(code: &str, body: Vec<u8>, pause: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/brief/", listener.local_addr().unwrap());
        let request = Arc::new(Mutex::new(String::new()));
        let captured = request.clone();
        let code = code.to_owned();
        let done = thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(3);
            let mut stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < until =>
                    {
                        thread::sleep(Duration::from_millis(5));
                    }
                    _ => return,
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") && bytes.len() < 8192 {
                let mut part = [0u8; 512];
                match stream.read(&mut part) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => bytes.extend_from_slice(&part[..n]),
                }
            }
            *captured.lock().unwrap() = String::from_utf8_lossy(&bytes).into_owned();
            thread::sleep(pause);
            let header = format!("HTTP/1.1 {code}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nLocation: http://127.0.0.1:1/never\r\nConnection: close\r\n\r\n", body.len());
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
        });
        Self {
            url,
            request,
            done: Some(done),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.done.take().unwrap().join().unwrap();
    }
}
fn identity() -> Value {
    json!({"project":"rikuy","revision":"a".repeat(40),"generated_at":"2026-10-28T00:00:00Z"})
}
fn check(server: &Server) -> site::Site {
    site::probe(
        Some(&server.url),
        Some(&curl_path()),
        Duration::from_millis(900),
    )
}
type Snapshot = Vec<(PathBuf, u64, u32, std::time::SystemTime, Vec<u8>)>;

fn snapshot(root: &Path) -> Snapshot {
    fn visit(root: &Path, out: &mut Snapshot) {
        let meta = std::fs::metadata(root).unwrap();
        out.push((
            root.to_owned(),
            meta.len(),
            meta.permissions().mode(),
            meta.modified().unwrap(),
            if meta.is_file() {
                std::fs::read(root).unwrap()
            } else {
                vec![]
            },
        ));
        if meta.is_dir() {
            for entry in std::fs::read_dir(root).unwrap() {
                visit(&entry.unwrap().path(), out);
            }
        }
    }
    let mut out = vec![];
    visit(root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn source_identity_and_generated_at_are_observed_by_one_get() {
    let server = Server::new(
        "200 OK",
        identity().to_string().into_bytes(),
        Duration::ZERO,
    );
    let result = check(&server);
    assert_eq!(result.status, "ok");
    assert_eq!(result.reachable, Some(true));
    assert_eq!(result.generated_at.as_deref(), Some("2026-10-28T00:00:00Z"));
    assert!(result.elapsed_ms < 1000);
    assert!(server
        .request
        .lock()
        .unwrap()
        .starts_with("GET /brief/source.json HTTP/1.1\r\n"));
}

#[test]
fn current_producer_without_time_stays_reachable_with_unknown_generation() {
    let mut body = identity();
    body.as_object_mut().unwrap().remove("generated_at");
    let server = Server::new("200 OK", body.to_string().into_bytes(), Duration::ZERO);
    let result = check(&server);
    assert_eq!(result.status, "ok");
    assert_eq!(result.reachable, Some(true));
    assert!(result.generated_at.is_none());
    assert!(result.line().contains("generated-at: unknown"));
}

#[test]
fn malformed_foreign_oversized_and_redirects_never_report_a_healthy_site() {
    let mut wrong = identity();
    wrong["project"] = json!("foreign");
    let mut bad_time = identity();
    bad_time["generated_at"] = json!("2026-02-30T25:00:00Z");
    for (code, body) in [
        ("200 OK", b"not JSON".to_vec()),
        ("200 OK", vec![255]),
        ("200 OK", wrong.to_string().into_bytes()),
        ("200 OK", bad_time.to_string().into_bytes()),
        ("200 OK", vec![b'x'; 70 * 1024]),
        ("302 Found", vec![]),
    ] {
        let server = Server::new(code, body, Duration::ZERO);
        let result = check(&server);
        assert_eq!(result.status, "unreadable", "{result:?}");
    }
    let server = Server::new("503 Unavailable", vec![], Duration::ZERO);
    assert_eq!(check(&server).status, "unreachable");
}

#[test]
fn refused_connection_and_slow_headers_have_distinct_bounded_results() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let result = site::probe(
        Some(&format!("http://{addr}/")),
        Some(&curl_path()),
        Duration::from_millis(900),
    );
    assert_eq!(result.status, "unreachable");
    assert_eq!(result.reachable, Some(false));
    let server = Server::new(
        "200 OK",
        identity().to_string().into_bytes(),
        Duration::from_millis(1200),
    );
    let started = Instant::now();
    let result = check(&server);
    assert_eq!(result.status, "partial");
    assert_eq!(result.reason, "deadline");
    assert!(result.elapsed_ms < 1000);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn absent_and_invalid_urls_do_not_start_transport_or_disclose_credentials() {
    let missing = OsString::from("/nonexistent/private-fixture-bin");
    assert_eq!(
        site::probe(None, Some(&missing), Duration::from_secs(1)).status,
        "absent"
    );
    for url in [
        "https://user:secret@example.test/",
        "file:///private",
        "https://example.test/?token=secret",
        "http://example.test/\n",
    ] {
        let result = site::probe(Some(url), Some(&missing), Duration::from_secs(1));
        assert_eq!(result.status, "unreadable");
        assert!(result.url.is_none());
        assert!(!result.value().to_string().contains("secret"));
    }
    let result = site::probe(
        Some("https://example.test/"),
        Some(&missing),
        Duration::from_secs(1),
    );
    assert_eq!(result.reason, "curl_not_available");
}

#[test]
fn compiled_cli_renders_site_separately_and_preserves_private_home_and_cwd() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let bin = private_curl();
    std::fs::write(
        home.path().join(".curlrc"),
        "url = http://127.0.0.1:1/never\noutput = forbidden\n",
    )
    .unwrap();
    let before = (snapshot(home.path()), snapshot(cwd.path()));
    for args in [
        vec!["--json"],
        vec!["--brief", "--json"],
        vec!["--brief"],
        vec![],
    ] {
        let server = Server::new(
            "200 OK",
            identity().to_string().into_bytes(),
            Duration::ZERO,
        );
        let out = Command::cargo_bin("tambo")
            .unwrap()
            .env("HOME", home.path())
            .env("PATH", bin.path())
            .env_remove("RIKUY_URL")
            .current_dir(cwd.path())
            .args(["prime", "--rikuy-url", &server.url])
            .args(&args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).unwrap();
        if args.contains(&"--json") {
            let value: Value = serde_json::from_str(&text).unwrap();
            let sites = &value["live"]["summary"]["sites"];
            assert_eq!(sites[0]["tool"], "rikuy");
            assert_eq!(sites[0]["kind"], "site");
            assert_eq!(sites[0]["reachable"], true, "{sites}");
            assert!(!value["live"]["summary"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["tool"] == "rikuy"));
            if args.contains(&"--brief") {
                assert!(value["primer"].get("text").is_none());
            }
        } else {
            assert!(text.contains("Rikuy site: reachable; generated-at: 2026-10-28T00:00:00Z"));
            if args.contains(&"--brief") {
                assert!((5..=10).contains(&text.lines().count()));
            }
        }
        assert_eq!(before, (snapshot(home.path()), snapshot(cwd.path())));
    }
    let out = Command::cargo_bin("tambo")
        .unwrap()
        .env("RIKUY_URL", "http://127.0.0.1:1/")
        .env("PATH", bin.path())
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .args(["prime", "--static", "--json"])
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["live"]["status"], "skipped");
    assert_eq!(before, (snapshot(home.path()), snapshot(cwd.path())));
}

#[test]
fn environment_url_works_and_explicit_url_takes_precedence() {
    let bin = private_curl();
    for explicit in [false, true] {
        let server = Server::new(
            "200 OK",
            identity().to_string().into_bytes(),
            Duration::ZERO,
        );
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let mut cmd = Command::cargo_bin("tambo").unwrap();
        cmd.env("PATH", bin.path())
            .env("HOME", home.path())
            .current_dir(cwd.path())
            .env(
                "RIKUY_URL",
                if explicit {
                    "file:///not-allowed"
                } else {
                    &server.url
                },
            )
            .args(["prime", "--brief", "--json"]);
        if explicit {
            cmd.args(["--rikuy-url", &server.url]);
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success());
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            value["live"]["summary"]["sites"][0]["reachable"], true,
            "{value}"
        );
    }
}

#[test]
fn unconfigured_optional_site_does_not_degrade_the_stack() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    for args in [vec!["--json"], vec!["--brief", "--json"], vec!["--brief"]] {
        let out = Command::cargo_bin("tambo")
            .unwrap()
            .env_remove("RIKUY_URL")
            .env("PATH", cwd.path())
            .env("HOME", home.path())
            .current_dir(cwd.path())
            .arg("prime")
            .args(&args)
            .output()
            .unwrap();
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        if args.contains(&"--json") {
            let v: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(v["status"], "ok", "{v}");
            assert_eq!(v["live"]["status"], "ok", "{v}");
            assert_eq!(v["live"]["summary"]["sites"][0]["status"], "absent");
            assert_eq!(v["live"]["summary"]["sites"][0]["configured"], false);
        } else {
            assert!(text.contains("Rikuy site: not configured"));
        }
    }
}

#[test]
fn configured_failure_categories_are_not_fabricated_deadlines() {
    let bin = private_curl();
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let mut no_time = identity();
    no_time.as_object_mut().unwrap().remove("generated_at");
    for (code, body, expected) in [
        ("503 Unavailable", vec![], "unreachable"),
        ("200 OK", b"not JSON".to_vec(), "unreadable"),
        ("200 OK", no_time.to_string().into_bytes(), "ok"),
    ] {
        let server = Server::new(code, body, Duration::ZERO);
        let out = Command::cargo_bin("tambo")
            .unwrap()
            .env_remove("RIKUY_URL")
            .env("PATH", bin.path())
            .env("HOME", home.path())
            .current_dir(cwd.path())
            .args(["prime", "--json", "--rikuy-url", &server.url])
            .output()
            .unwrap();
        assert!(out.status.success());
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["live"]["status"], expected, "{v}");
        assert_eq!(
            v["status"],
            if expected == "ok" { "ok" } else { "degraded" }
        );
    }
}
