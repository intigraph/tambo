//! Rikuy is a static site, not a program on PATH. Read only its source identity.
use std::ffi::OsStr;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{run_bounded, READ_CAP};
use crate::adapter::which_all;

#[derive(Debug)]
pub struct Site {
    pub url: Option<String>,
    pub status: &'static str,
    pub reachable: Option<bool>,
    pub generated_at: Option<String>,
    pub revision: Option<String>,
    pub reason: &'static str,
    pub elapsed_ms: u128,
}

impl Site {
    pub fn value(&self) -> Value {
        json!({"tool":"rikuy","kind":"site","url":self.url,"status":self.status,
            "reachable":self.reachable,"generated_at":self.generated_at,"revision":self.revision,
            "reason":self.reason,"elapsed_ms":self.elapsed_ms,"scope":"static-site-identity",
            "live_brief":"not_measured","configured":self.reason != "url_not_configured","next":"tambo prime --rikuy-url <site-url>"})
    }

    pub fn line(&self) -> String {
        let status = if self.reason == "url_not_configured" {
            "not configured"
        } else if self.reachable == Some(true) {
            "reachable"
        } else {
            self.status
        };
        format!(
            "Rikuy site: {status}; generated-at: {}; source: {}",
            self.generated_at.as_deref().unwrap_or("unknown"),
            self.url
                .as_deref()
                .unwrap_or("not configured (set RIKUY_URL or --rikuy-url)")
        )
    }
}

// No credentials, query/fragment, URL globbing or control characters can enter
// curl or its output. A site may live under a path prefix.
fn source_url(value: &str) -> Option<String> {
    if value.len() > 2048
        || value
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        || value.contains(['?', '#', '@', '\\'])
    {
        return None;
    }
    let (scheme, rest) = value.split_once("://")?;
    let authority = rest.split('/').next()?;
    if !matches!(scheme, "http" | "https")
        || authority.is_empty()
        || !authority
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".:-[]".contains(&b))
    {
        return None;
    }
    Some(if value.ends_with("/source.json") {
        value.to_owned()
    } else {
        format!("{}/source.json", value.trim_end_matches('/'))
    })
}

// The producer has no generation time today. Accept a UTC RFC3339 timestamp
// when a producer supplies one; never substitute HTTP Date or our check time.
fn timestamp(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() < 20 || b.len() > 30 || b.last() != Some(&b'Z') {
        return false;
    }
    for i in [4, 7, 10, 13, 16] {
        if b[i]
            != match i {
                4 | 7 => b'-',
                10 => b'T',
                _ => b':',
            }
        {
            return false;
        }
    }
    if !b[..19]
        .iter()
        .enumerate()
        .all(|(i, c)| [4, 7, 10, 13, 16].contains(&i) || c.is_ascii_digit())
    {
        return false;
    }
    if b.len() > 20
        && (b[19] != b'.' || b.len() < 22 || !b[20..b.len() - 1].iter().all(u8::is_ascii_digit))
    {
        return false;
    }
    let number = |a, z| value[a..z].parse::<u32>().unwrap_or(u32::MAX);
    let (year, month, day) = (number(0, 4), number(5, 7), number(8, 10));
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1..=12 => 31,
        _ => 0,
    };
    year > 0
        && day > 0
        && day <= days
        && number(11, 13) < 24
        && number(14, 16) < 60
        && number(17, 19) < 60
}

pub fn probe(url: Option<&str>, path: Option<&OsStr>, timeout: Duration) -> Site {
    let started = Instant::now();
    let mut site = Site {
        url: None,
        status: "absent",
        reachable: None,
        generated_at: None,
        revision: None,
        reason: "url_not_configured",
        elapsed_ms: 0,
    };
    let Some(raw) = url else {
        return site;
    };
    let Some(url) = source_url(raw) else {
        site.status = "unreadable";
        site.reason = "invalid_url_redacted";
        return site;
    };
    site.url = Some(url.clone());
    let Some(curl) = which_all("curl", path).into_iter().next() else {
        site.status = "unreadable";
        site.reason = "curl_not_available";
        return site;
    };
    // Keep below 1s including process launch and collection. run_bounded kills
    // the whole process group at the deadline, including stalled DNS/TLS.
    let budget = timeout.min(Duration::from_millis(900));
    let seconds = format!("{:.3}", budget.as_secs_f64());
    let cap = READ_CAP.to_string();
    let args = [
        "-q",
        "--silent",
        "--globoff",
        "--proxy",
        "",
        "--noproxy",
        "*",
        "--proto",
        "=http,https",
        "--retry",
        "0",
        "--max-redirs",
        "0",
        "--connect-timeout",
        &seconds,
        "--max-time",
        &seconds,
        "--max-filesize",
        &cap,
        "--request",
        "GET",
        "--write-out",
        "\n%{http_code}",
        "--url",
        &url,
    ];
    let result = run_bounded(&curl, &args, None, false, started + budget);
    site.elapsed_ms = started.elapsed().as_millis();
    let (status, bytes) = match result {
        None => {
            site.status = "partial";
            site.reason = "deadline";
            return site;
        }
        Some(Err(())) => {
            site.status = "unreadable";
            site.reason = "curl_not_started";
            return site;
        }
        Some(Ok(out)) => out,
    };
    if !status.success() {
        site.status = match status.code() {
            Some(28) => "partial",
            Some(23 | 63) => "unreadable",
            _ => "unreachable",
        };
        site.reason = if status.code() == Some(28) {
            "deadline"
        } else {
            "transport_failed"
        };
        if site.status == "unreachable" {
            site.reachable = Some(false);
        }
        return site;
    }
    site.status = "unreadable";
    site.reason = "invalid_source_identity";
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return site;
    };
    let Some((body, code)) = text.rsplit_once('\n') else {
        return site;
    };
    if code != "200" {
        site.status = if code.starts_with('3') {
            "unreadable"
        } else {
            "unreachable"
        };
        site.reason = if code.starts_with('3') {
            "redirect_refused"
        } else {
            "http_error"
        };
        if site.status == "unreachable" {
            site.reachable = Some(false);
        }
        return site;
    }
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return site;
    };
    let Some(revision) = value["revision"].as_str() else {
        return site;
    };
    if value["project"] != "rikuy"
        || revision.len() != 40
        || !revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return site;
    }
    site.reachable = Some(true);
    site.revision = Some(revision.to_owned());
    match value.get("generated_at") {
        None | Some(Value::Null) => {
            // The deployed producer does not supply this optional timestamp.
            // Identity is reachable; unknown generation is not a deadline.
            site.status = "ok";
            site.reason = "generated_at_unknown";
        }
        Some(Value::String(at)) if timestamp(at) => {
            site.generated_at = Some(at.to_owned());
            site.status = "ok";
            site.reason = "source_identity_read";
        }
        _ => {
            site.reason = "invalid_generated_at";
        }
    }
    site
}
