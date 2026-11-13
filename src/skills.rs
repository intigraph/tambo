//! Explicit local skill publication/materialization. Never edits harness settings.
//! Catalogue files are caller-supplied inputs, not authenticated server evidence.
mod native;
mod publish;
mod sync;

use anyhow::{Context, Result};
use clap::Subcommand;
use serde::de::DeserializeOwned;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Subcommand)]
pub enum Command {
    /// Build a native pendant and release metadata from one explicit source tree
    Publish {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        metadata: PathBuf,
        #[arg(long)]
        render_shapes: PathBuf,
        #[arg(long, requires = "identifier_shapes")]
        identifier_policy: Option<PathBuf>,
        #[arg(long, requires = "identifier_policy")]
        identifier_shapes: Option<PathBuf>,
        /// A NEW output directory; existing releases are never replaced
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "quipu")]
        quipu: PathBuf,
    },
    /// Render a complete pinned profile into ONE owned staging directory
    Sync {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        catalogue: PathBuf,
        /// Explicit local map: native share ID -> archive path (never URLs)
        #[arg(long)]
        bundles: PathBuf,
        /// Caller-selected native shape authority, not authority from a skill
        #[arg(long)]
        authority: PathBuf,
        #[arg(long)]
        render_shapes: PathBuf,
        /// Owned profile tree, NOT a shared harness root or settings file
        #[arg(long)]
        dest: PathBuf,
        #[arg(long, default_value = "quipu")]
        quipu: PathBuf,
    },
}

pub fn run(command: Command) -> Result<()> {
    let result = match command {
        Command::Publish {
            source,
            metadata,
            render_shapes,
            identifier_policy,
            identifier_shapes,
            output,
            quipu,
        } => publish::publish(
            &source,
            &metadata,
            &render_shapes,
            identifier_policy
                .as_deref()
                .zip(identifier_shapes.as_deref()),
            &output,
            &quipu,
        )?,
        Command::Sync {
            profile,
            catalogue,
            bundles,
            authority,
            render_shapes,
            dest,
            quipu,
        } => sync::sync(
            &profile,
            &catalogue,
            &bundles,
            &authority,
            &render_shapes,
            &dest,
            &quipu,
        )?,
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = native::regular_bytes(path, 4 * 1024 * 1024)?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid skill input: {}", path.display()))
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

#[cfg(test)]
mod tests;
