//! Shell completion generator.
use clap::CommandFactory;
use clap_complete::generate;

use crate::cli::{Cli, CompletionArgs};
use crate::error::Result;

pub fn execute(args: &CompletionArgs) -> Result<()> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(args.shell, &mut cmd, name, &mut std::io::stdout());
    Ok(())
}
