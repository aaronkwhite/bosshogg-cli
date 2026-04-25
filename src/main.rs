//! BossHogg binary entry point.
//!
//! Responsibilities:
//! 1. Initialize `tracing` subscriber from `BOSSHOGG_LOG` env var (falls back to `RUST_LOG`).
//! 2. Parse CLI args.
//! 3. Dispatch to the matching command module (Part 2 wires this up).
//! 4. Render errors via `output::print_error` and exit with `err.exit_code()`.

use bosshogg::commands::context::CommandContext;
use bosshogg::{analytics, cli::Cli, output};
use clap::Parser;
use std::time::{Duration, Instant};
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() {
    init_tracing();

    let cli = Cli::parse();
    let json_mode = cli.json;

    // Capture telemetry inputs before `cli` is moved into `run`. The
    // command-name lookup and flag-vector build both need a reference to
    // `cli` while it's still intact; region is read from on-disk config
    // (no network).
    let telemetry_command = cli.command.as_ref().map(analytics::command_name);
    let telemetry_flags = collect_flags(&cli);
    let telemetry_region = analytics::is_enabled()
        .then(|| bosshogg::config::active_region(cli.context.as_deref()))
        .flatten();

    let start = Instant::now();
    let result = run(cli).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    // Build + queue the event before printing errors / exiting. Skip when
    // there's no subcommand (clap auto-prints help) or when this was a
    // local-only completion script render.
    if let Some(command) = telemetry_command
        && command != "completion"
    {
        let (success, error_code, exit_code) = match &result {
            Ok(()) => (true, None, None),
            Err(err) => (
                false,
                Some(err.error_code().to_string()),
                Some(err.exit_code()),
            ),
        };
        analytics::track(&analytics::Event {
            command: command.to_string(),
            flags: telemetry_flags,
            success,
            duration_ms,
            region: telemetry_region,
            error_code,
            exit_code,
        });
    }

    // Fire-and-forget flush, capped at 3 s so a slow / unreachable
    // PostHog can't stall command exit.
    let flush_handle = tokio::spawn(analytics::flush());
    let _ = tokio::time::timeout(Duration::from_secs(3), flush_handle).await;

    match result {
        Ok(()) => std::process::exit(0),
        Err(err) => {
            output::print_error(&err, json_mode);
            std::process::exit(err.exit_code());
        }
    }
}

/// Top-level flag presence vector for telemetry. Names only — never values
/// (`--api-key` and `--host` are intentionally omitted to avoid even
/// hinting at sensitive material).
fn collect_flags(cli: &Cli) -> Vec<String> {
    let mut flags = Vec::new();
    if cli.json {
        flags.push("--json".into());
    }
    if cli.debug {
        flags.push("--debug".into());
    }
    if cli.yes {
        flags.push("--yes".into());
    }
    if cli.context.is_some() {
        flags.push("--context".into());
    }
    flags
}

/// Build a `CommandContext` from the parsed CLI flags. Calling convention:
/// pass individual fields rather than `&Cli` so the borrow checker is
/// happy after `cli.command` has been partially moved by the match arm.
fn ctx(
    json: bool,
    debug: bool,
    context: Option<&str>,
    yes: bool,
) -> bosshogg::Result<CommandContext> {
    CommandContext::new(json, debug, context, yes)
}

/// Dispatch to the matching command module.
async fn run(cli: Cli) -> bosshogg::Result<()> {
    use bosshogg::cli::Commands;
    use bosshogg::commands;

    // Snapshot the context-field borrow before the match consumes cli.command.
    // cli.json/debug/yes are Copy; cli.context needs .as_deref() each arm.
    let json = cli.json;
    let debug = cli.debug;
    let yes = cli.yes;

    match cli.command {
        // --- Migrated commands: each arm builds CommandContext via ctx() ---
        Some(Commands::Whoami) => {
            commands::whoami::execute(&ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Schema(args)) => {
            commands::schema::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Auth(args)) => {
            commands::auth::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Query(args)) => {
            commands::query::execute(&args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Flag(args)) => {
            commands::flag::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Insight(args)) => {
            commands::insight::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::Dashboard(args)) => {
            commands::dashboard::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::Cohort(args)) => {
            commands::cohort::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Org(args)) => {
            commands::org::execute(&args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Project(args)) => {
            commands::project::execute(&args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::Person(args)) => {
            commands::person::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Group(args)) => {
            commands::group::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Event(args)) => {
            commands::event::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Action(args)) => {
            commands::action::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Annotation(args)) => {
            commands::annotation::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::EventDefinition(args)) => {
            commands::event_definition::execute(
                args,
                &ctx(json, debug, cli.context.as_deref(), yes)?,
            )
            .await?
        }
        Some(Commands::PropertyDefinition(args)) => {
            commands::property_definition::execute(
                args,
                &ctx(json, debug, cli.context.as_deref(), yes)?,
            )
            .await?
        }
        Some(Commands::Endpoint(args)) => {
            commands::endpoint::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::Experiment(args)) => {
            commands::experiment::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::Survey(args)) => {
            commands::survey::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::EarlyAccess(args)) => {
            commands::early_access::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::HogFunction(args)) => {
            commands::hog_function::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::BatchExport(args)) => {
            commands::batch_export::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::SessionRecording(args)) => {
            commands::session_recording::execute(
                args,
                &ctx(json, debug, cli.context.as_deref(), yes)?,
            )
            .await?
        }
        Some(Commands::ErrorTracking(args)) => {
            commands::error_tracking::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::Role(args)) => {
            commands::role::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Alert(args)) => {
            commands::alert::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::DashboardTemplate(args)) => {
            commands::dashboard_template::execute(
                args,
                &ctx(json, debug, cli.context.as_deref(), yes)?,
            )
            .await?
        }
        Some(Commands::SessionRecordingPlaylist(args)) => {
            commands::session_recording_playlist::execute(
                args,
                &ctx(json, debug, cli.context.as_deref(), yes)?,
            )
            .await?
        }
        Some(Commands::InsightVariable(args)) => {
            commands::insight_variable::execute(
                args,
                &ctx(json, debug, cli.context.as_deref(), yes)?,
            )
            .await?
        }
        Some(Commands::Dataset(args)) => {
            commands::dataset::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::DatasetItem(args)) => {
            commands::dataset_item::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::Evaluation(args)) => {
            commands::evaluation::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::LlmAnalytics(args)) => {
            commands::llm_analytics::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }

        // --- Excluded commands: keep original primitive signatures ---
        Some(Commands::Configure(args)) => commands::configure::execute(args, json, debug).await?,
        Some(Commands::Doctor(args)) => {
            commands::doctor::execute(args, json, debug, cli.context.as_deref()).await?
        }
        Some(Commands::Capture(args)) => {
            commands::capture::execute(args, json, debug, cli.context.as_deref(), yes).await?
        }
        Some(Commands::Use(args)) => commands::use_cmd::execute(args, json).await?,
        Some(Commands::Completion(args)) => bosshogg::commands::completion::execute(&args)?,
        Some(Commands::Config(args)) => commands::config::run(args, json).await?,
        None => {
            // No subcommand: clap will print help automatically.
        }
    }
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_env("BOSSHOGG_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("bosshogg=warn"));
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_ansi(console::Term::stderr().is_term())
        .try_init();
}
