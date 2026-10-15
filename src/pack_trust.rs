//! Pendant autoload trust (aegis-2eckx0): which packs and pointer endpoints may load.
//!
//! A pack lands in the graph agents query first, so loading it is a trust decision
//! (design: `docs/book/src/pendant-autoload.md`, section 2):
//!
//! - **Your own repositories load automatically**: their remote owner is on the
//!   `owners` list.
//! - **A foreign pack needs an explicit allow**, recorded per repository AND pack
//!   hash. A repository name in a configuration file is not consent, so an entry
//!   without a hash is refused when the file is read, not silently honoured.
//! - **A foreign pointer endpoint needs its own allow** (repository + endpoint).
//!   Declaring an endpoint inside a foreign pack never authorises it.
//!
//! Trust decides only whether a pack may be composed at all. It never promotes
//! anything to ROOT, and it does not choose shape authority: autoload composes with
//! the store's own shapes, which is the caller's contract with quipu.
//!
//! The decisions live in their own file, separate from the shared stack config, so
//! every grant is in one auditable place.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// `~/.config/caboodle/pack-trust.toml`.
pub fn default_path() -> Result<PathBuf> {
    let home = env::var_os("HOME").context("HOME is unset; cannot locate pack trust config")?;
    Ok(PathBuf::from(home).join(".config/caboodle/pack-trust.toml"))
}

/// The whole trust file.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustConfig {
    /// Remote owners (`host/owner`, e.g. `github.com/scbrown`) whose packs autoload.
    #[serde(default)]
    pub owners: Vec<String>,
    /// Foreign packs admitted one exact archive at a time.
    #[serde(default, rename = "allow")]
    pub packs: Vec<PackAllow>,
    /// Foreign pointer endpoints admitted one exact URL at a time.
    #[serde(default, rename = "allow_endpoint")]
    pub endpoints: Vec<EndpointAllow>,
}

/// One admitted foreign pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackAllow {
    /// Repository as `host/owner/name`.
    pub repo: String,
    /// sha256 of the exact pack archive, lowercase hex.
    pub pack_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One admitted foreign endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointAllow {
    pub repo: String,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The outcome of one trust question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Loads; `basis` names the rule that admitted it.
    Allow { basis: String },
    /// Refused; `message` names the pack (or endpoint) and how to admit it.
    Refuse { message: String },
}

impl Decision {
    pub fn allowed(&self) -> bool {
        matches!(self, Decision::Allow { .. })
    }
}

/// A repository identity: `host/owner/name`, lowercased.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub host: String,
    pub owner: String,
    pub name: String,
}

impl Repo {
    /// Parse a git remote (`https://host/owner/name(.git)`, `ssh://git@host/owner/name`,
    /// `git@host:owner/name.git`) or a plain `host/owner/name`.
    pub fn parse(remote: &str) -> Result<Repo> {
        let trimmed = remote.trim().trim_end_matches('/');
        let rest = if let Some((scheme, rest)) = trimmed.split_once("://") {
            if !matches!(scheme, "https" | "http" | "ssh" | "git") {
                bail!("unsupported remote scheme in {remote:?}");
            }
            rest.rsplit_once('@')
                .map_or(rest, |(_, after)| after)
                .to_string()
        } else if let Some((user_host, path)) = trimmed.split_once(':') {
            // scp-like: git@host:owner/name
            let host = user_host.rsplit_once('@').map_or(user_host, |(_, h)| h);
            format!("{host}/{path}")
        } else {
            trimmed.to_string()
        };
        let rest = rest.strip_suffix(".git").unwrap_or(&rest);
        let parts: Vec<&str> = rest.split('/').collect();
        let [host, owner, name] = parts.as_slice() else {
            bail!("remote {remote:?} is not host/owner/name");
        };
        // A port belongs to the transport, not the identity.
        let host = host.split(':').next().unwrap_or(host);
        let valid = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        };
        if !(valid(host) && valid(owner) && valid(name)) {
            bail!("remote {remote:?} has an invalid host, owner or name");
        }
        Ok(Repo {
            host: host.to_ascii_lowercase(),
            owner: owner.to_ascii_lowercase(),
            name: name.to_ascii_lowercase(),
        })
    }

    pub fn owner_key(&self) -> String {
        format!("{}/{}", self.host, self.owner)
    }

    pub fn key(&self) -> String {
        format!("{}/{}/{}", self.host, self.owner, self.name)
    }
}

fn check_sha256(value: &str) -> Result<String> {
    let lower = value.trim().to_ascii_lowercase();
    if lower.len() != 64 || !lower.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("pack hash {value:?} is not a 64-character sha256");
    }
    Ok(lower)
}

fn normalize_owner(owner: &str) -> Result<String> {
    let parts: Vec<&str> = owner.trim().trim_matches('/').split('/').collect();
    let [host, name] = parts.as_slice() else {
        bail!("owner {owner:?} must be host/owner, e.g. github.com/scbrown");
    };
    Ok(Repo::parse(&format!("{host}/{name}/x"))?.owner_key())
}

impl TrustConfig {
    /// Read and validate. A missing file trusts nothing. An invalid one refuses
    /// everything: a trust file that cannot be read is not permission.
    pub fn load(path: &Path) -> Result<TrustConfig> {
        if !path.exists() {
            return Ok(TrustConfig::default());
        }
        let body = fs::read_to_string(path)
            .with_context(|| format!("read pack trust {}", path.display()))?;
        let config: TrustConfig = toml::from_str(&body).with_context(|| {
            format!(
                "parse pack trust {}; every pack is refused until it is fixed (an allow \
                 entry needs repo AND pack_sha256)",
                path.display()
            )
        })?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        for owner in &self.owners {
            normalize_owner(owner)?;
        }
        for allow in &self.packs {
            Repo::parse(&allow.repo)?;
            check_sha256(&allow.pack_sha256)?;
        }
        for allow in &self.endpoints {
            Repo::parse(&allow.repo)?;
            check_endpoint(&allow.endpoint)?;
        }
        Ok(())
    }

    /// Atomic write with read-back, like the stack config.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let rendered = toml::to_string_pretty(self).context("render pack trust")?;
        let parent = path.parent().context("pack trust path has no parent")?;
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        let temp = path.with_extension("toml.tmp");
        fs::write(&temp, &rendered).with_context(|| format!("write {}", temp.display()))?;
        fs::rename(&temp, path).with_context(|| format!("replace {}", path.display()))?;
        if &TrustConfig::load(path)? != self {
            bail!("pack trust read-back did not match what was written");
        }
        Ok(())
    }

    /// May the pack archive with this sha256, shipped by the repository at `remote`, load?
    pub fn decide_pack(&self, remote: &str, pack: &str, pack_sha256: &str) -> Decision {
        let (repo, sha) = match (Repo::parse(remote), check_sha256(pack_sha256)) {
            (Ok(repo), Ok(sha)) => (repo, sha),
            (Err(e), _) | (_, Err(e)) => {
                return Decision::Refuse {
                    message: format!("pack {pack} refused: {e}"),
                }
            }
        };
        if self.owns(&repo) {
            return Decision::Allow {
                basis: format!(
                    "own repository ({} is on the owners list)",
                    repo.owner_key()
                ),
            };
        }
        let pinned = self.packs.iter().any(|a| {
            Repo::parse(&a.repo).is_ok_and(|r| r == repo)
                && check_sha256(&a.pack_sha256).is_ok_and(|s| s == sha)
        });
        if pinned {
            return Decision::Allow {
                basis: format!("explicit allow for {} at sha256 {sha}", repo.key()),
            };
        }
        Decision::Refuse {
            message: format!(
                "foreign pack {pack} (sha256 {sha}) from {} has no allow entry. Admit exactly \
                 this archive with: caboodle packs trust allow {} {sha}",
                repo.key(),
                repo.key()
            ),
        }
    }

    /// May a pointer endpoint declared by the pack of the repository at `remote` be federated?
    pub fn decide_endpoint(&self, remote: &str, endpoint: &str) -> Decision {
        let (repo, url) = match (Repo::parse(remote), check_endpoint(endpoint)) {
            (Ok(repo), Ok(url)) => (repo, url),
            (Err(e), _) | (_, Err(e)) => {
                return Decision::Refuse {
                    message: format!("endpoint {endpoint} refused: {e}"),
                }
            }
        };
        if self.owns(&repo) {
            return Decision::Allow {
                basis: format!(
                    "own repository ({} is on the owners list)",
                    repo.owner_key()
                ),
            };
        }
        let allowed = self.endpoints.iter().any(|a| {
            Repo::parse(&a.repo).is_ok_and(|r| r == repo)
                && check_endpoint(&a.endpoint).is_ok_and(|e| e == url)
        });
        if allowed {
            return Decision::Allow {
                basis: format!("explicit endpoint allow for {}", repo.key()),
            };
        }
        Decision::Refuse {
            message: format!(
                "endpoint {url} declared by foreign repository {} has no allow entry; a pack \
                 declaring it is not consent. Admit it with: caboodle packs trust \
                 allow-endpoint {} {url}",
                repo.key(),
                repo.key()
            ),
        }
    }

    fn owns(&self, repo: &Repo) -> bool {
        self.owners
            .iter()
            .any(|o| normalize_owner(o).is_ok_and(|o| o == repo.owner_key()))
    }

    /// Add an owner; idempotent.
    pub fn add_owner(&mut self, owner: &str) -> Result<bool> {
        let key = normalize_owner(owner)?;
        if self
            .owners
            .iter()
            .any(|o| normalize_owner(o).is_ok_and(|o| o == key))
        {
            return Ok(false);
        }
        self.owners.push(key);
        Ok(true)
    }

    /// Admit one exact foreign pack; idempotent.
    pub fn add_pack(&mut self, repo: &str, pack_sha256: &str, note: Option<&str>) -> Result<bool> {
        let repo = Repo::parse(repo)?.key();
        let sha = check_sha256(pack_sha256)?;
        if self
            .packs
            .iter()
            .any(|a| a.repo == repo && a.pack_sha256 == sha)
        {
            return Ok(false);
        }
        self.packs.push(PackAllow {
            repo,
            pack_sha256: sha,
            note: note.map(str::to_string),
        });
        Ok(true)
    }

    /// Admit one exact foreign endpoint; idempotent.
    pub fn add_endpoint(&mut self, repo: &str, endpoint: &str, note: Option<&str>) -> Result<bool> {
        let repo = Repo::parse(repo)?.key();
        let url = check_endpoint(endpoint)?;
        if self
            .endpoints
            .iter()
            .any(|a| a.repo == repo && a.endpoint == url)
        {
            return Ok(false);
        }
        self.endpoints.push(EndpointAllow {
            repo,
            endpoint: url,
            note: note.map(str::to_string),
        });
        Ok(true)
    }
}

/// An endpoint is an exact `http(s)` URL with no credentials in it.
fn check_endpoint(endpoint: &str) -> Result<String> {
    let url = endpoint.trim();
    let Some((scheme, rest)) = url.split_once("://") else {
        bail!("endpoint {endpoint:?} is not an http(s) URL");
    };
    if !matches!(scheme, "https" | "http") || rest.is_empty() {
        bail!("endpoint {endpoint:?} is not an http(s) URL");
    }
    let authority = rest.split('/').next().unwrap_or_default();
    if authority.contains('@') {
        bail!("endpoint {endpoint:?} carries credentials; trust is recorded without secrets");
    }
    Ok(url.to_string())
}

#[cfg(test)]
#[path = "pack_trust_tests.rs"]
mod tests;
