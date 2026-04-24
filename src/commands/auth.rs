use clap::{Args, Subcommand};

use crate::commands::context::CommandContext;
use crate::error::Result;

#[derive(Args, Debug)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// Emit the active context's personal API key to stdout.
    Token,
}

pub async fn execute(args: AuthArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        AuthCommand::Token => {
            // We only need the key; use the client so all the resolution
            // rules (env vars, precedence, redaction-in-debug) apply.
            // Emit raw, no newline suffix beyond println.
            println!("{}", cx.client.api_key());
            Ok(())
        }
    }
}
