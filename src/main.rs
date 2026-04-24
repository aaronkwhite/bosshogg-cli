//! BossHogg binary entry point.
//!
//! Responsibilities:
//! 1. Initialize `tracing` subscriber from `BOSSHOGG_LOG` env var (falls back to `RUST_LOG`).
//! 2. Parse CLI args.
//! 3. Dispatch to the matching command module (Part 2 wires this up).
//! 4. Render errors via `output::print_error` and exit with `err.exit_code()`.

use bosshogg::commands::context::CommandContext;
use bosshogg::{cli::Cli, output};
use clap::Parser;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() {
    init_tracing();

    let cli = Cli::parse();
    let json_mode = cli.json;

    match run(cli).await {
        Ok(()) => std::process::exit(0),
        Err(err) => {
            output::print_error(&err, json_mode);
            std::process::exit(err.exit_code());
        }
    }
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
            commands::schema::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::Auth(args)) => {
            commands::auth::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }
        Some(Commands::Query(args)) => {
            commands::query::execute(&args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
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
            commands::early_access::execute(
                args,
                &ctx(json, debug, cli.context.as_deref(), yes)?,
            )
            .await?
        }
        Some(Commands::HogFunction(args)) => {
            commands::hog_function::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?)
                .await?
        }
        Some(Commands::BatchExport(args)) => {
            commands::batch_export::execute(
                args,
                &ctx(json, debug, cli.context.as_deref(), yes)?,
            )
            .await?
        }
        Some(Commands::Subscription(args)) => {
            commands::subscription::execute(
                args,
                &ctx(json, debug, cli.context.as_deref(), yes)?,
            )
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
            commands::error_tracking::execute(
                args,
                &ctx(json, debug, cli.context.as_deref(), yes)?,
            )
            .await?
        }
        Some(Commands::Role(args)) => {
            commands::role::execute(args, &ctx(json, debug, cli.context.as_deref(), yes)?).await?
        }

        // --- Excluded commands: keep original primitive signatures ---
        Some(Commands::Configure(args)) => {
            commands::configure::execute(args, json, debug).await?
        }
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
