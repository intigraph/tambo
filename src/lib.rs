pub mod adapter;
pub mod apt_owned;
pub mod configuration;
pub mod crew;
pub mod doctor;
pub mod dp_alias;
pub mod embedding;
pub mod emission;
pub mod engine;
pub mod hook_bundles;
pub mod interview;
#[cfg(unix)]
pub mod member_bump;
pub mod members;
pub mod model;
pub mod observability;
pub mod pack_trust;
pub mod package;
pub mod pendant_hooks;
pub mod pendant_ownership;
pub mod pendant_projects;
pub mod pendant_sync;
pub mod prime;
pub mod projection;
pub mod skill_profile;
pub mod tool_bundles;

#[cfg(unix)]
pub mod release_update;

mod quipu_auth;
mod quipu_credential;
#[cfg(unix)]
pub mod quipu_mcp;
#[cfg(all(test, unix))]
mod test_exec;
pub mod vocabulary;
