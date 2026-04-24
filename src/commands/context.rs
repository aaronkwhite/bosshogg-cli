//! `CommandContext` — the single value threaded through command handlers.
//!
//! Replaces the ad-hoc `(client, json_mode, yes)` tuple that used to appear
//! in every internal handler signature. Owned by the top-level
//! `execute()` of each command module and passed by reference (`&cx`) to
//! sub-handlers.
//!
//! Intentionally narrow: no `debug: bool` field (it's consumed at
//! `Client::new` construction and baked into the `Client`). `context_name`
//! IS carried — `org switch` and `project switch` mutate a specific
//! config context and need the `--context` override to route to the
//! right one.

use crate::client::Client;
use crate::commands::util::gate_destructive;
use crate::Result;

/// Shared handler context. Owns the `Client`; carries per-invocation
/// flags (`json_mode`, `yes`) and the `--context` override for handlers
/// that mutate config (`org switch`, `project switch`).
pub struct CommandContext {
    pub client: Client,
    pub json_mode: bool,
    pub yes: bool,
    /// Name of the config context the user chose via `--context`. `None`
    /// means "use the currently active context from config". Only
    /// `org switch` and `project switch` read this; every other handler
    /// gets its resolved Client from `self.client`.
    pub context_name: Option<String>,
}

impl CommandContext {
    /// Build a context from parsed CLI flags. `debug` is consumed by
    /// `Client::new` (baked into the `Client`). `context` is stored so
    /// switch-arm handlers can find the right config section to mutate.
    pub fn new(
        json_mode: bool,
        debug: bool,
        context: Option<&str>,
        yes: bool,
    ) -> Result<Self> {
        let client = Client::new(context, debug)?;
        Ok(Self {
            client,
            json_mode,
            yes,
            context_name: context.map(String::from),
        })
    }

    /// Destructive-action gate. Identical semantics to
    /// `gate_destructive(self.yes, prompt)`.
    pub fn confirm(&self, prompt: &str) -> Result<()> {
        gate_destructive(self.yes, prompt)
    }

    /// Convenience accessor so handlers read `cx.is_json()` instead of
    /// reaching into the field directly. Returning `self.json_mode`
    /// verbatim — not a reserved name for future logic.
    pub fn is_json(&self) -> bool {
        self.json_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ResolvedAuth;

    fn test_client_stub() -> Client {
        // Client::for_test skips https_only and config-file loading.
        // We can build a minimal ResolvedAuth entirely in-process.
        let auth = ResolvedAuth {
            api_key: "phc_test_key".to_string(),
            host: "https://us.posthog.com".to_string(),
            project_id: None,
            env_id: None,
            org_id: None,
            context_name: None,
        };
        Client::for_test(auth, false).expect("test client build should not fail")
    }

    #[test]
    fn confirm_passes_through_when_yes_true() {
        let cx = CommandContext {
            client: test_client_stub(),
            json_mode: false,
            yes: true,
            context_name: None,
        };
        assert!(cx.confirm("delete everything?").is_ok());
    }

    #[test]
    fn is_json_mirrors_field() {
        let cx = CommandContext {
            client: test_client_stub(),
            json_mode: true,
            yes: false,
            context_name: None,
        };
        assert!(cx.is_json());
    }
}
