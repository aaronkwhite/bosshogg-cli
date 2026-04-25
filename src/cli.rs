//! Top-level Clap derive tree.
//!
//! Part 2 (subagent) extends the `Commands` enum with one variant per
//! resource (`Flag`, `Query`, `Configure`, `Whoami`, `Doctor`, `Schema`,
//! `Auth`, `Config`, `Use`, `Completion`, `Version`). Keep the global
//! options on `Cli` stable — they flow through every command.

use clap::{Parser, Subcommand};

use crate::commands::{
    action::ActionArgs, alert::AlertArgs, annotation::AnnotationArgs, auth::AuthArgs,
    batch_export::BatchExportArgs, capture::CaptureArgs, cohort::CohortArgs, config::ConfigArgs,
    configure::ConfigureArgs, dashboard::DashboardArgs, dashboard_template::DashboardTemplateArgs,
    dataset::DatasetArgs, dataset_item::DatasetItemArgs, doctor::DoctorArgs,
    early_access::EarlyAccessArgs, endpoint::EndpointArgs, error_tracking::ErrorTrackingArgs,
    evaluation::EvaluationArgs, event::EventArgs, event_definition::EventDefinitionArgs,
    experiment::ExperimentArgs, flag::FlagArgs, group::GroupArgs, hog_function::HogFunctionArgs,
    insight::InsightArgs, insight_variable::InsightVariableArgs, llm_analytics::LlmAnalyticsArgs,
    org::OrgArgs, person::PersonArgs, project::ProjectArgs,
    property_definition::PropertyDefinitionArgs, query::QueryArgs, role::RoleArgs,
    schema::SchemaArgs, session_recording::SessionRecordingArgs,
    session_recording_playlist::SessionRecordingPlaylistArgs, survey::SurveyArgs, use_cmd::UseArgs,
};

#[derive(Parser, Debug)]
#[command(
    name = "bosshogg",
    author,
    version,
    about = "Agent-first PostHog CLI",
    long_about = None,
    propagate_version = true,
)]
pub struct Cli {
    /// Emit compact JSON instead of tables. Auto-enabled when stdout is not a TTY.
    #[arg(long, global = true)]
    pub json: bool,

    /// Verbose debug output to stderr (redacts auth headers, truncates bodies).
    #[arg(long, global = true)]
    pub debug: bool,

    /// Named context to use for this invocation (overrides `current_context`).
    #[arg(long, short = 'c', global = true)]
    pub context: Option<String>,

    /// One-off API key override (highest priority in auth resolution chain).
    #[arg(long, global = true, env = "POSTHOG_CLI_TOKEN", hide_env_values = true)]
    pub api_key: Option<String>,

    /// One-off host override.
    #[arg(long, global = true)]
    pub host: Option<String>,

    /// Skip confirmation prompts on destructive operations (updates, deletes, rotations,
    /// enable/disable, and other mutations). Required for non-interactive use where prompts
    /// would block.
    #[arg(long, short = 'y', global = true)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Configure(ConfigureArgs),
    Whoami,
    Doctor(DoctorArgs),
    Schema(SchemaArgs),
    Auth(AuthArgs),
    Config(ConfigArgs),
    Query(QueryArgs),
    Flag(FlagArgs),
    Insight(InsightArgs),
    Dashboard(DashboardArgs),
    Cohort(CohortArgs),
    Org(OrgArgs),
    Project(ProjectArgs),
    Person(PersonArgs),
    Group(GroupArgs),
    Event(EventArgs),
    Action(ActionArgs),
    Annotation(AnnotationArgs),
    #[command(name = "event-definition")]
    EventDefinition(EventDefinitionArgs),
    #[command(name = "property-definition")]
    PropertyDefinition(PropertyDefinitionArgs),
    Endpoint(EndpointArgs),
    Experiment(ExperimentArgs),
    Survey(SurveyArgs),
    #[command(name = "early-access")]
    EarlyAccess(EarlyAccessArgs),
    #[command(name = "hog-function")]
    HogFunction(HogFunctionArgs),
    #[command(name = "batch-export")]
    BatchExport(BatchExportArgs),
    #[command(name = "session-recording")]
    SessionRecording(SessionRecordingArgs),
    #[command(name = "error-tracking")]
    ErrorTracking(ErrorTrackingArgs),
    Role(RoleArgs),
    Capture(CaptureArgs),
    Alert(AlertArgs),
    #[command(name = "dashboard-template")]
    DashboardTemplate(DashboardTemplateArgs),
    #[command(name = "session-recording-playlist")]
    SessionRecordingPlaylist(SessionRecordingPlaylistArgs),
    #[command(name = "insight-variable")]
    InsightVariable(InsightVariableArgs),
    Dataset(DatasetArgs),
    #[command(name = "dataset-item")]
    DatasetItem(DatasetItemArgs),
    Evaluation(EvaluationArgs),
    #[command(name = "llm-analytics")]
    LlmAnalytics(LlmAnalyticsArgs),
    #[command(name = "use")]
    Use(UseArgs),
    /// Generate shell completions for bash/zsh/fish/powershell
    Completion(CompletionArgs),
}

#[derive(clap::Args, Debug)]
pub struct CompletionArgs {
    /// Target shell (bash, zsh, fish, powershell, elvish)
    pub shell: clap_complete::Shell,
}
