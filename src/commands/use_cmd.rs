use clap::Args;

use crate::commands::config::{self as cfg_cmd};
use crate::error::Result;

#[derive(Args, Debug)]
pub struct UseArgs {
    /// Context name to switch to.
    pub name: String,
}

// No `debug` or `context` parameters: `use` is a pure config-local operation
// that writes the active context name to disk with no API call.
pub async fn execute(args: UseArgs, json_mode: bool) -> Result<()> {
    cfg_cmd::run(
        cfg_cmd::ConfigArgs {
            command: cfg_cmd::ConfigCommand::UseContext { name: args.name },
        },
        json_mode,
    )
    .await
}
