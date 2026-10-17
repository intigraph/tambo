use std::path::PathBuf;

use anyhow::Result;
use caboodle::{
    crew::CrewEvidence,
    doctor,
    embedding::EmbeddingModel,
    emission, engine, interview,
    model::{CrewMode, InstallIntent, Plan, Profile, QuipuFlavor, State},
    observability, projection,
};
use clap::{Parser, Subcommand, ValueEnum};

/// `0.2.2 (1a2b3c4d5e6f)`: the release version and the commit it was built from.
const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("CABOODLE_GIT_SHA"),
    ")"
);

#[derive(Parser)]
#[command(name = env!("CARGO_BIN_NAME"), version = VERSION, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install an administrator-issued Quipu token after proving acceptance; never rotate
    ProvisionQuipuToken {
        #[arg(long = "from")]
        source: PathBuf,
        #[arg(long)]
        server: String,
    },
    /// Report, without changing anything, what would stop an install on this host
    Doctor {
        /// Plan to check; without one, every tool in the everything profile is checked
        #[arg(short, long, default_value = "caboodle-plan.toml")]
        plan: PathBuf,
        /// Install state; when it records every planned tool verified, doctor says so
        #[arg(long, default_value = ".caboodle/state.json")]
        state: PathBuf,
    },
    /// Run the resumable guided interview and write a reviewable plan
    Init {
        /// Use the guided question flow
        #[arg(long)]
        guided: bool,
        #[arg(short, long, default_value = "caboodle-plan.toml")]
        output: PathBuf,
        #[arg(long, default_value = ".caboodle/interview.toml")]
        session: PathBuf,
        /// Opt in to repository pendant sync hooks; retains existing hooks
        #[arg(long, conflicts_with = "guided")]
        pack_hooks: bool,
        /// Select project shares by their native path scope in installed hooks
        #[arg(long, requires = "pack_hooks")]
        monorepo: bool,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Operator-controlled shape-authority pendant for local composition
        #[arg(long, requires = "pack_hooks")]
        shapes_from: Option<PathBuf>,
        #[arg(long, requires = "pack_hooks")]
        db: Option<PathBuf>,
        #[arg(long, requires = "pack_hooks")]
        trust_file: Option<PathBuf>,
    },
    /// Write a reviewable install plan without changing the machine
    Plan {
        #[arg(long, value_enum, default_value_t = ProfileArg::Retrieval)]
        profile: ProfileArg,
        /// Crew runtime when --profile crew is selected
        #[arg(long, value_enum, default_value_t = CrewModeArg::Standalone)]
        crew: CrewModeArg,
        /// Reviewable intended-use, crew-theme, and ontology-question contract
        #[arg(long, default_value = "caboodle-intent.toml")]
        intent: PathBuf,
        #[arg(short, long, default_value = "caboodle-plan.toml")]
        output: PathBuf,
        /// Canonical Quipu share directories to stage during apply
        #[arg(long = "share")]
        shares: Vec<PathBuf>,
        /// Explicit Quipu database that receives staged shares
        #[arg(long, requires = "shares")]
        quipu_db: Option<PathBuf>,
        /// Quipu install flavor; lancedb builds the same reviewed revision
        /// with the lancedb cargo feature compiled in
        #[arg(long, value_enum, default_value_t = QuipuFlavorArg::Release)]
        quipu_flavor: QuipuFlavorArg,
        /// Checksum-pinned embedding-model artifact spec copied into the plan
        #[arg(long)]
        embedding_model: Option<PathBuf>,
        /// Register this Quipu server's MCP endpoint with Claude Code through a
        /// headersHelper that reads the token at connect time (no secret in config)
        #[arg(long)]
        quipu_mcp_url: Option<String>,
        /// This host's Quipu token FILE, baked into the helper as its default
        /// (a path, never the token). Defaults to $QUIPU_AUTH_TOKEN_FILE when set.
        #[arg(long, requires = "quipu_mcp_url")]
        quipu_mcp_token_file: Option<String>,
    },
    /// Converge installed tools and prove each binary by version read-back
    Apply {
        #[arg(short, long, default_value = "caboodle-plan.toml")]
        plan: PathBuf,
        #[arg(long, default_value = ".caboodle/state.json")]
        state: PathBuf,
        /// Refuse to install missing tools; useful for managed or offline machines
        #[arg(long)]
        skip_install: bool,
        /// Apply this plan host profile (e.g. exclude crew a different lane owns)
        #[arg(long)]
        host_profile: Option<String>,
    },
    /// Run isolated functional round trips for every selected tool
    Verify {
        #[arg(short, long, default_value = "caboodle-plan.toml")]
        plan: PathBuf,
        #[arg(long, default_value = ".caboodle/state.json")]
        state: PathBuf,
        /// Creel browser doctor JSON produced by its capability preflight
        #[arg(long)]
        creel_doctor: Option<PathBuf>,
        /// Creel admission JSON produced by its provider-window governor
        #[arg(long)]
        creel_admission: Option<PathBuf>,
        /// Apply this plan host profile (e.g. exclude crew a different lane owns)
        #[arg(long)]
        host_profile: Option<String>,
    },
    /// Verify one embedded release member in temporary roots without installing it
    VerifyMember { member: String },
    /// Apply and verify a previously reviewed plan
    Install {
        #[arg(short, long, default_value = "caboodle-plan.toml")]
        plan: PathBuf,
        #[arg(long, default_value = ".caboodle/state.json")]
        state: PathBuf,
        #[arg(long)]
        skip_install: bool,
        #[arg(long)]
        creel_doctor: Option<PathBuf>,
        #[arg(long)]
        creel_admission: Option<PathBuf>,
        /// Apply this plan host profile (e.g. exclude crew a different lane owns)
        #[arg(long)]
        host_profile: Option<String>,
    },
    /// Report drift from the releases reviewed by this Caboodle build
    CheckUpdates {
        #[arg(short, long, default_value = "caboodle-plan.toml")]
        plan: PathBuf,
        /// Apply this plan host profile (e.g. exclude crew a different lane owns)
        #[arg(long)]
        host_profile: Option<String>,
    },
    /// Converge drifted tools to reviewed releases and verify each change
    Update {
        #[arg(short, long, default_value = "caboodle-plan.toml")]
        plan: PathBuf,
        #[arg(long, default_value = ".caboodle/state.json")]
        state: PathBuf,
        #[arg(long)]
        creel_doctor: Option<PathBuf>,
        #[arg(long)]
        creel_admission: Option<PathBuf>,
        /// Apply this plan host profile (e.g. exclude crew a different lane owns)
        #[arg(long)]
        host_profile: Option<String>,
    },
    /// Update this installer from a published checksummed release
    #[cfg(unix)]
    UpdateSelf {
        #[arg(long, default_value = ".caboodle/state.json")]
        state: PathBuf,
        #[arg(long)]
        check: bool,
        /// Select this exact published stable installer tag instead of latest.
        #[arg(long)]
        tag: Option<String>,
        /// Bind the downloaded archive to pre-install release evidence.
        #[arg(long, requires_all = ["tag", "binary_sha256"])]
        archive_sha256: Option<String>,
        /// Bind candidate bytes before executing or replacing the installer.
        #[arg(long, requires_all = ["tag", "archive_sha256"])]
        binary_sha256: Option<String>,
    },
    /// Internal: update-self runs this on the NEWLY installed binary so its
    /// own repairs apply on the first natural update (aegis-61f).
    #[cfg(unix)]
    #[command(hide = true)]
    PostInstallRepair,
    /// Maintainers: re-pin a stack member manifest to its published release,
    /// recording the digests the release publishes after hashing each asset
    /// (review and merge the result; installs never read the release's sums)
    #[cfg(unix)]
    BumpMember {
        /// The manifest to rewrite, e.g. members/seeds.toml
        manifest: PathBuf,
        /// Release tag to pin instead of the latest stable release
        #[arg(long)]
        tag: Option<String>,
        /// Run every execution of the new, unreviewed release as
        /// `<wrapper> <program> <args...>`, e.g. a sandbox (aegis-uy26l7)
        #[arg(long)]
        probe_wrapper: Option<PathBuf>,
    },
    /// Track a published binary release for one tool in an existing reviewed plan
    #[cfg(unix)]
    UpdateRelease {
        #[arg(short, long, default_value = "caboodle-plan.toml")]
        plan: PathBuf,
        #[arg(long, default_value = ".caboodle/state.json")]
        state: PathBuf,
        /// bobbin, yupana or desire-path track the newest published release;
        /// quipu and stack members track reviewed pins. Camayoc/Shantytown
        /// refuse until checksummed publications exist
        #[arg(long, value_parser = update_release_tool)]
        tool: String,
        /// Inspect release/version metadata without downloading or replacing binaries
        #[arg(long)]
        check: bool,
    },
    /// Restore Seeds from the receipt of its last successful release update
    #[cfg(unix)]
    RollbackSeeds {
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        current_sha256: String,
        #[arg(long)]
        prior_sha256: String,
    },
    /// Register the rig crew through shantytown and its Quipu tooling manifest
    ProjectSettings {
        /// Shantytown store root (otherwise st uses its normal discovery)
        #[arg(long)]
        root: Option<PathBuf>,
        /// Equip only this local crew card (default: every local card)
        #[arg(long)]
        agent: Option<String>,
        #[arg(long, value_parser = ["files", "quipu", "toml"], default_value = "files")]
        registry: String,
        /// Only export the legacy plan policy summaries; does not register MCP
        #[arg(long, conflicts_with_all = ["root", "agent"])]
        policy_only: bool,
        #[arg(
            short,
            long,
            default_value = "caboodle-plan.toml",
            requires = "policy_only"
        )]
        plan: PathBuf,
        #[arg(
            short,
            long,
            default_value = "caboodle-settings",
            requires = "policy_only"
        )]
        output: PathBuf,
    },
    /// Execute every anticipated ontology question against the installed Quipu
    VerifyQuestions {
        #[arg(short, long, default_value = "caboodle-plan.toml")]
        plan: PathBuf,
        /// Optional isolated Quipu database used by the question probes
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Render reviewable Prometheus, alert, dashboard, and contract artifacts
    RenderObservability {
        #[arg(short, long, default_value = "caboodle-plan.toml")]
        plan: PathBuf,
        #[arg(long, default_value = "caboodle-observability.toml")]
        targets: PathBuf,
        #[arg(short, long, default_value = "caboodle-observability")]
        output: PathBuf,
    },
    /// Validate generated observability artifacts against their contracts
    ValidateObservability {
        #[arg(short, long, default_value = "caboodle-observability")]
        output: PathBuf,
    },
    /// Queue created/commented/closed lifecycle episodes from a br JSONL snapshot
    QueueBr {
        input: PathBuf,
        #[arg(short, long, default_value = ".caboodle/episodes")]
        queue: PathBuf,
    },
    /// Deliver identical queued episodes after a Quipu readiness control passes
    FlushEpisodes {
        #[arg(short, long, default_value = ".caboodle/episodes")]
        queue: PathBuf,
        #[arg(long)]
        endpoint: String,
    },
    /// Pendant packs a repository ships under .quipu/packs/
    Packs {
        #[command(subcommand)]
        command: PacksCommand,
    },
}

#[derive(Subcommand)]
enum PacksCommand {
    /// Print the native CONSTRUCT path scope used for one derived project share
    ProjectScope {
        /// Read CODEOWNERS from this checkout and attach ownership in the scope
        #[arg(long, requires_all = ["ownership_predicate", "db"])]
        repo: Option<PathBuf>,
        /// Existing producer store; owner resources must already be registered
        #[arg(long, requires = "repo")]
        db: Option<PathBuf>,
        /// Existing RDF ownership predicate, supplied by the producer's schema
        #[arg(long, requires = "repo")]
        ownership_predicate: Option<String>,
        #[arg(long)]
        graph: String,
        #[arg(long)]
        path_predicate: String,
        #[arg(long)]
        prefix: String,
    },
    /// Discover trusted .quipu/packs/*.pendant.tar.gz and update local CURRENT
    Sync {
        /// Exclude the parent and load only derived projects present in the checkout
        #[arg(long)]
        monorepo: bool,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long)]
        db: Option<PathBuf>,
        /// Trusted local authority pendant, never chosen from repository data
        #[arg(long)]
        shapes_from: Option<PathBuf>,
        #[arg(long)]
        trust_file: Option<PathBuf>,
    },
    /// Which packs and pointer endpoints may load (~/.config/caboodle/pack-trust.toml)
    Trust {
        /// Trust file to read and write instead of the default
        #[arg(long, global = true)]
        file: Option<PathBuf>,
        #[command(subcommand)]
        command: TrustCommand,
    },
}

#[derive(Subcommand)]
enum TrustCommand {
    /// Autoload every repository of this owner (host/owner, e.g. github.com/scbrown)
    Own { owner: String },
    /// Admit exactly one foreign pack archive: repository plus its sha256
    Allow {
        repo: String,
        sha256: String,
        #[arg(long)]
        note: Option<String>,
    },
    /// Admit exactly one pointer endpoint declared by a foreign repository's pack
    AllowEndpoint {
        repo: String,
        endpoint: String,
        #[arg(long)]
        note: Option<String>,
    },
    /// Print the trust file
    List,
    /// Decide one pack: exit 0 to load, 3 refused (message names the pack)
    Check {
        /// The repository's git remote URL
        remote: String,
        sha256: String,
        /// The pack's path, named in a refusal
        #[arg(long, default_value = "pack")]
        pack: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ProfileArg {
    Kg,
    Retrieval,
    CodeIntel,
    Crew,
    Everything,
}

#[derive(Clone, Copy, ValueEnum)]
enum QuipuFlavorArg {
    Release,
    Lancedb,
}

impl From<QuipuFlavorArg> for QuipuFlavor {
    fn from(value: QuipuFlavorArg) -> Self {
        match value {
            QuipuFlavorArg::Release => Self::Release,
            QuipuFlavorArg::Lancedb => Self::Lancedb,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum CrewModeArg {
    Shantytown,
    Creel,
    Both,
    Standalone,
}

impl From<CrewModeArg> for CrewMode {
    fn from(value: CrewModeArg) -> Self {
        match value {
            CrewModeArg::Shantytown => Self::Shantytown,
            CrewModeArg::Creel => Self::Creel,
            CrewModeArg::Both => Self::Both,
            CrewModeArg::Standalone => Self::Standalone,
        }
    }
}

impl From<ProfileArg> for Profile {
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::Kg => Self::Kg,
            ProfileArg::Retrieval => Self::Retrieval,
            ProfileArg::CodeIntel => Self::CodeIntel,
            ProfileArg::Crew => Self::Crew,
            ProfileArg::Everything => Self::Everything,
        }
    }
}

/// `update-release --tool`: the release-tracked built-ins, or any embedded member.
#[cfg(unix)]
fn update_release_tool(value: &str) -> Result<String, String> {
    if [
        "bobbin",
        "yupana",
        "desire-path",
        "quipu",
        "camayoc",
        "shantytown",
    ]
    .contains(&value)
        || caboodle::members::get(value).is_some()
    {
        Ok(value.to_owned())
    } else {
        let members: Vec<&str> = caboodle::members::all()
            .iter()
            .map(|m| m.name.as_str())
            .collect();
        Err(format!(
            "possible values: bobbin, yupana, desire-path, quipu, camayoc, shantytown, or a stack member ({})",
            members.join(", ")
        ))
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::ProvisionQuipuToken { source, server } => {
            engine::provision_quipu_credential(&source, &server)?;
        }
        Commands::Doctor { plan, state } => {
            let mut installed = false;
            let scope = if plan.exists() {
                println!("doctor: checking {}", plan.display());
                let read = Plan::read(&plan)?;
                installed = doctor::installed(&read, &State::read(&state)?);
                let mut scope = doctor::Scope::from_plan(&read);
                scope.work_dir = doctor::is_work_dir(&plan);
                scope
            } else {
                println!(
                    "doctor: no plan at {}; checking every tool in the everything profile",
                    plan.display()
                );
                doctor::Scope::everything()
            };
            let findings = doctor::diagnose(&scope);
            let stdout = std::io::stdout();
            if !doctor::report(&findings, installed, &mut stdout.lock())? {
                std::process::exit(1);
            }
        }
        Commands::Init {
            guided,
            output,
            session,
            pack_hooks,
            monorepo,
            repo,
            shapes_from,
            db,
            trust_file,
        } => {
            if pack_hooks {
                let options = caboodle::pendant_sync::Options {
                    repo: &repo,
                    db: db.as_deref(),
                    shapes_from: shapes_from.as_deref(),
                    trust_file: trust_file.as_deref(),
                    monorepo,
                };
                for path in caboodle::pendant_hooks::install(&options)? {
                    println!("pendant hook: {}", path.display());
                }
                return Ok(());
            }
            if !guided {
                anyhow::bail!("caboodle init currently requires --guided");
            }
            let stdin = std::io::stdin();
            let mut input = stdin.lock();
            let stdout = std::io::stdout();
            let mut output_stream = stdout.lock();
            interview::guided(&mut input, &mut output_stream, &session, &output)?;
        }
        Commands::Plan {
            profile,
            crew,
            intent,
            output,
            shares,
            quipu_db,
            quipu_flavor,
            embedding_model,
            quipu_mcp_url,
            quipu_mcp_token_file,
        } => {
            let profile = Profile::from(profile);
            let crew: CrewMode = crew.into();
            let mut plan = if profile == Profile::Crew {
                Plan::for_crew(crew)
            } else {
                let mut plan = Plan::for_profile(profile);
                if profile == Profile::Everything && crew != CrewMode::Standalone {
                    plan.crew = Some(caboodle::model::CrewSelection::for_mode(crew));
                }
                plan
            };
            plan.shares = shares;
            plan.quipu_db = quipu_db;
            plan.quipu_flavor = quipu_flavor.into();
            if let Some(spec) = embedding_model {
                plan.embedding_model = Some(EmbeddingModel::read(&spec)?);
            }
            plan.quipu_mcp = quipu_mcp_url.map(|url| caboodle::model::QuipuMcp {
                url,
                token_file: quipu_mcp_token_file.or_else(|| {
                    std::env::var("QUIPU_AUTH_TOKEN_FILE")
                        .ok()
                        .filter(|path| !path.is_empty())
                }),
            });
            plan.intent = Some(InstallIntent::read(&intent)?);
            plan.write(&output)?;
            println!("plan: {}", output.display());
        }
        Commands::Apply {
            plan,
            state,
            skip_install,
            host_profile,
        } => {
            engine::apply(
                &read_plan(&plan, host_profile.as_deref())?,
                &state,
                skip_install,
            )?;
        }
        Commands::Verify {
            plan,
            state,
            creel_doctor,
            creel_admission,
            host_profile,
        } => {
            engine::verify(
                &read_plan(&plan, host_profile.as_deref())?,
                &state,
                &CrewEvidence {
                    creel_doctor,
                    creel_admission,
                },
            )?;
        }
        Commands::VerifyMember { member } => {
            caboodle::adapter::verify_member(&member)?;
            println!("{member}: reviewed release functional verification passed");
        }
        Commands::Install {
            plan,
            state,
            skip_install,
            creel_doctor,
            creel_admission,
            host_profile,
        } => {
            let plan = read_plan(&plan, host_profile.as_deref())?;
            engine::apply(&plan, &state, skip_install)?;
            engine::verify(
                &plan,
                &state,
                &CrewEvidence {
                    creel_doctor,
                    creel_admission,
                },
            )?;
        }
        Commands::CheckUpdates { plan, host_profile } => {
            let plan = read_plan(&plan, host_profile.as_deref())?;
            if !engine::check_updates(&plan)? {
                anyhow::bail!("selected stack has pending reviewed updates");
            }
        }
        Commands::Update {
            plan,
            state,
            creel_doctor,
            creel_admission,
            host_profile,
        } => {
            engine::update(
                &read_plan(&plan, host_profile.as_deref())?,
                &state,
                &CrewEvidence {
                    creel_doctor,
                    creel_admission,
                },
            )?;
        }
        #[cfg(unix)]
        Commands::UpdateSelf {
            state,
            check,
            tag,
            archive_sha256,
            binary_sha256,
        } => match (tag, archive_sha256, binary_sha256) {
            (Some(tag), Some(archive), Some(binary)) => {
                caboodle::release_update::update_self_pinned(
                    &state, check, &tag, &archive, &binary,
                )?
            }
            (Some(tag), None, None) => {
                caboodle::release_update::update_self_at(&state, check, &tag)?
            }
            (None, None, None) => caboodle::release_update::update_self(&state, check)?,
            _ => anyhow::bail!("installer artifact binding requires tag and both SHA256 values"),
        },
        #[cfg(unix)]
        Commands::PostInstallRepair => {
            println!(
                "post-install repair: {}",
                caboodle::adapter::repair_owned_quipu_alias()?
            );
        }
        #[cfg(unix)]
        Commands::BumpMember {
            manifest,
            tag,
            probe_wrapper,
        } => {
            match caboodle::member_bump::bump(&manifest, tag.as_deref(), probe_wrapper.as_deref())?
            {
                caboodle::member_bump::Outcome::Current { version } => {
                    println!(
                        "{}: already pins {version}; nothing written",
                        manifest.display()
                    );
                }
                caboodle::member_bump::Outcome::Bumped { from, to, digests } => {
                    println!("{}: {from} -> {to}", manifest.display());
                    for (target, digest) in digests {
                        println!("  {target} {digest}");
                    }
                    println!(
                        "review the diff and open a PR; it ships in the next caboodle release"
                    );
                }
            }
        }
        #[cfg(unix)]
        Commands::UpdateRelease {
            plan,
            state,
            tool,
            check,
        } => {
            if tool == "shantytown" {
                anyhow::bail!(
                    "shantytown publishes a wheel without a checksum asset; release update refused"
                );
            }
            let tool = serde_json::from_value(serde_json::Value::String(tool))?;
            caboodle::release_update::update(&Plan::read(&plan)?, tool, &state, check)?;
        }
        #[cfg(unix)]
        Commands::RollbackSeeds {
            plan,
            state,
            current_sha256,
            prior_sha256,
        } => {
            caboodle::release_update::rollback_seeds(
                &Plan::read(&plan)?,
                &state,
                &current_sha256,
                &prior_sha256,
            )?;
        }
        Commands::ProjectSettings {
            plan,
            output,
            root,
            agent,
            registry,
            policy_only,
        } => {
            if policy_only {
                for name in projection::write(&Plan::read(&plan)?, &output)? {
                    println!(
                        "policy summary (not MCP registration): {}",
                        output.join(name).display()
                    );
                }
            } else {
                projection::register(root.as_deref(), agent.as_deref(), &registry)?;
            }
        }
        Commands::VerifyQuestions { plan, db } => {
            engine::verify_questions(&Plan::read(&plan)?, db.as_deref())?;
        }
        Commands::RenderObservability {
            plan,
            targets,
            output,
        } => {
            observability::render(&Plan::read(&plan)?, &targets, &output)?;
            println!("observability: {}", output.display());
        }
        Commands::ValidateObservability { output } => {
            observability::validate(&output)?;
            println!("observability contracts: verified");
        }
        Commands::QueueBr { input, queue } => {
            let count = emission::queue_br_jsonl(&input, &queue)?;
            println!("queued lifecycle projections: {count}");
        }
        Commands::FlushEpisodes { queue, endpoint } => {
            let count = emission::flush(&queue, &endpoint)?;
            println!("delivered queued episodes: {count}");
        }
        Commands::Packs {
            command:
                PacksCommand::Sync {
                    monorepo,
                    repo,
                    db,
                    shapes_from,
                    trust_file,
                },
        } => {
            let result = caboodle::pendant_sync::sync(&caboodle::pendant_sync::Options {
                repo: &repo,
                db: db.as_deref(),
                shapes_from: shapes_from.as_deref(),
                trust_file: trust_file.as_deref(),
                monorepo,
            })?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Packs {
            command:
                PacksCommand::ProjectScope {
                    graph,
                    path_predicate,
                    prefix,
                    repo,
                    ownership_predicate,
                    db,
                },
        } => {
            let owners = if let (Some(repo), Some(predicate)) = (repo, ownership_predicate) {
                caboodle::pendant_ownership::resolve(&repo, &prefix, &predicate)?
            } else {
                Vec::new()
            };
            let query = caboodle::pendant_projects::enriched_query(
                &graph,
                &path_predicate,
                &prefix,
                &owners,
            )?;
            if let Some(db) = db {
                caboodle::pendant_ownership::verify_owners(&graph, &db, &owners)?;
            }
            println!("{query}");
        }
        Commands::Packs {
            command: PacksCommand::Trust { file, command },
        } => run_pack_trust(file, command)?,
    }
    Ok(())
}

fn run_pack_trust(file: Option<PathBuf>, command: TrustCommand) -> Result<()> {
    use caboodle::pack_trust::{default_path, Decision, TrustConfig};
    let path = match file {
        Some(path) => path,
        None => default_path()?,
    };
    let mut trust = TrustConfig::load(&path)?;
    let changed = match command {
        TrustCommand::Own { owner } => trust.add_owner(&owner)?,
        TrustCommand::Allow { repo, sha256, note } => {
            trust.add_pack(&repo, &sha256, note.as_deref())?
        }
        TrustCommand::AllowEndpoint {
            repo,
            endpoint,
            note,
        } => trust.add_endpoint(&repo, &endpoint, note.as_deref())?,
        TrustCommand::List => {
            print!("{}", toml::to_string_pretty(&trust)?);
            return Ok(());
        }
        TrustCommand::Check {
            remote,
            sha256,
            pack,
        } => {
            match trust.decide_pack(&remote, &pack, &sha256) {
                Decision::Allow { basis } => println!("load: {basis}"),
                Decision::Refuse { message } => {
                    eprintln!("refused: {message}");
                    std::process::exit(3);
                }
            }
            return Ok(());
        }
    };
    if changed {
        trust.save(&path)?;
        println!("recorded in {}", path.display());
    } else {
        println!("already recorded in {}", path.display());
    }
    Ok(())
}

/// Read a plan and apply `--host-profile`, printing each exclusion so an
/// excluded runtime is visible and never mistaken for a verified one.
fn read_plan(path: &std::path::Path, host_profile: Option<&str>) -> anyhow::Result<Plan> {
    let (plan, excluded) = Plan::read(path)?.for_host(host_profile)?;
    for line in excluded {
        println!("{line}");
    }
    Ok(plan)
}
