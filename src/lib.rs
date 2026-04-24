//! BossHogg — agent-first PostHog CLI.
//!
//! Library crate exposes internals for integration tests. Binary consumers
//! should invoke the `bosshogg` executable, not this crate.
//!
//! Module layout mirrors docs/architecture.md.

pub mod cli;
pub mod client;
pub mod commands;
pub mod config;
pub mod error;
pub mod output;
pub mod util;

pub use error::{BosshoggError, Result};
