//! JSON contract tests.
//!
//! Every BossHogg command that accepts `--json` must produce output that
//! validates against a schema in `tests/schemas/<surface>.schema.json`.
//!
//! We test two ways:
//! 1. Static fixtures in `tests/fixtures/` are checked against their schemas
//!    (guards the schemas against drift with our serde structs).
//! 2. Real command invocations against mocked hosts (covered in
//!    `tests/rest_shapes.rs`, not here).
//!
//! When a command's output shape changes, update both the schema and the
//! fixture in the same PR.

use crate::common::{assert_schema, load_fixture};

#[test]
fn harness_is_wired() {
    // Tiny smoke test: an obviously-valid payload against a tiny schema.
    // Proves the harness loads schemas and reports errors.
    let good = r#"{"ok":true}"#;
    // schemas/_harness.schema.json is the smallest possible schema (see G6).
    assert_schema(good, "_harness");
}

#[test]
fn flag_list_fixture_matches_schema() {
    let fixture = load_fixture("flag_list");
    let as_string = serde_json::to_string(&fixture).unwrap();
    assert_schema(&as_string, "flag_list");
}

#[test]
fn flag_get_fixture_matches_schema() {
    let fixture = load_fixture("flag_get");
    let as_string = serde_json::to_string(&fixture).unwrap();
    assert_schema(&as_string, "flag_get");
}

#[test]
fn whoami_fixture_matches_schema() {
    let fixture = load_fixture("users_me");
    let as_string = serde_json::to_string(&fixture).unwrap();
    assert_schema(&as_string, "whoami");
}

#[test]
fn doctor_fixture_matches_schema() {
    let fixture = load_fixture("doctor_checks");
    let as_string = serde_json::to_string(&fixture).unwrap();
    assert_schema(&as_string, "doctor");
}

#[test]
fn schema_hogql_fixture_matches_schema() {
    let fixture = load_fixture("hogql_schema");
    let as_string = serde_json::to_string(&fixture).unwrap();
    assert_schema(&as_string, "schema_hogql");
}

#[test]
fn query_run_fixture_matches_schema() {
    let fixture = load_fixture("query_response");
    let as_string = serde_json::to_string(&fixture).unwrap();
    assert_schema(&as_string, "query_run");
}

#[test]
fn error_envelope_fixture_matches_schema() {
    let fixture = load_fixture("error_envelope");
    let as_string = serde_json::to_string(&fixture).unwrap();
    assert_schema(&as_string, "error");
}

// Note: End-to-end fixture test via assert_cmd + wiremock is deferred to rest_shapes.rs
// which mocks at the serde level, avoiding the HTTPS-only client restriction in tests.
