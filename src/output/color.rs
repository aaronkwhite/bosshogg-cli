//! Color helpers — NO_COLOR-aware, TTY-aware. Thin wrappers over `console`.
//!
//! Never use `\x1b[...` escapes directly; always go through these helpers so
//! NO_COLOR actually disables color everywhere.

use console::{Style, Term, style};

fn enabled() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    Term::stdout().is_term() || Term::stderr().is_term()
}

pub fn red(s: &str) -> String {
    if enabled() {
        style(s).red().to_string()
    } else {
        s.to_string()
    }
}

pub fn green(s: &str) -> String {
    if enabled() {
        style(s).green().to_string()
    } else {
        s.to_string()
    }
}

pub fn yellow(s: &str) -> String {
    if enabled() {
        style(s).yellow().to_string()
    } else {
        s.to_string()
    }
}

pub fn cyan(s: &str) -> String {
    if enabled() {
        style(s).cyan().to_string()
    } else {
        s.to_string()
    }
}

pub fn bold(s: &str) -> String {
    if enabled() {
        style(s).bold().to_string()
    } else {
        s.to_string()
    }
}

pub fn dim(s: &str) -> String {
    if enabled() {
        Style::new().dim().apply_to(s).to_string()
    } else {
        s.to_string()
    }
}
