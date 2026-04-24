//! Interactive prompt helpers.
//!
//! Rules (docs/conventions.md § Interactive):
//! - Never prompt when stdout is not a TTY.
//! - Destructive actions auto-confirm in non-TTY mode.
//! - Fuzzy-select requires a TTY AND a missing optional flag.

use crate::output::is_interactive;

use dialoguer::{Confirm, FuzzySelect, Input};

use crate::error::{BosshoggError, Result};

pub fn confirm(prompt: &str, default_yes: bool) -> Result<bool> {
    if !is_interactive() {
        return Ok(true);
    }
    Confirm::new()
        .with_prompt(prompt)
        .default(default_yes)
        .interact()
        .map_err(|e| BosshoggError::Io(std::io::Error::other(e)))
}

pub fn input(prompt: &str, default: Option<&str>) -> Result<String> {
    if !is_interactive() {
        return Err(BosshoggError::BadRequest(format!(
            "{prompt} required; pass the corresponding flag (non-interactive mode)"
        )));
    }
    let mut builder = Input::<String>::new().with_prompt(prompt);
    if let Some(d) = default {
        builder = builder.default(d.to_string());
    }
    builder
        .interact_text()
        .map_err(|e| BosshoggError::Io(std::io::Error::other(e)))
}

pub fn pick(prompt: &str, items: &[String]) -> Result<usize> {
    if !is_interactive() {
        return Err(BosshoggError::BadRequest(format!(
            "{prompt} — multiple options; pass the flag explicitly (non-interactive mode)"
        )));
    }
    FuzzySelect::new()
        .with_prompt(prompt)
        .items(items)
        .default(0)
        .interact()
        .map_err(|e| BosshoggError::Io(std::io::Error::other(e)))
}
