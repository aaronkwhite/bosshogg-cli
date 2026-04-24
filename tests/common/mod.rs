//! Shared helpers for BossHogg integration tests.
//!
//! This file is intentionally small. Bigger helpers live next to the test
//! that needs them.
#![allow(dead_code)] // different test files use different subsets

pub mod harness;
#[allow(unused_imports)]
pub use harness::TestHarness;

use std::fs;
use std::path::Path;

use jsonschema::Validator;
use serde_json::Value;

/// Load a JSON Schema document from `tests/schemas/<name>.schema.json`.
pub fn load_schema(name: &str) -> Validator {
    let path = format!("tests/schemas/{name}.schema.json");
    let bytes = fs::read(&path).unwrap_or_else(|e| {
        panic!("failed to read schema {path}: {e}");
    });
    let doc: Value = serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!("schema {path} is not valid JSON: {e}");
    });
    jsonschema::validator_for(&doc).unwrap_or_else(|e| {
        panic!("schema {path} is not a valid JSON Schema: {e}");
    })
}

/// Load a JSON fixture from `tests/fixtures/<name>.json`.
pub fn load_fixture(name: &str) -> Value {
    let path = format!("tests/fixtures/{name}.json");
    let bytes = fs::read(&path).unwrap_or_else(|e| {
        panic!("failed to read fixture {path}: {e}");
    });
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!("fixture {path} is not valid JSON: {e}");
    })
}

/// Assert that `output` (raw stdout from a command) is JSON that satisfies
/// `schemas/<schema_name>.schema.json`.
///
/// On failure, prints every validation error before panicking.
pub fn assert_schema(output: &str, schema_name: &str) {
    let schema = load_schema(schema_name);
    let value: Value = serde_json::from_str(output).unwrap_or_else(|e| {
        panic!("output was not valid JSON ({e}): {output}");
    });
    let errors: Vec<_> = schema.iter_errors(&value).collect();
    if !errors.is_empty() {
        for err in &errors {
            eprintln!("schema violation: {err}");
        }
        panic!("output did not match schema {schema_name}");
    }
}

/// Assert that a file on disk contains JSON satisfying a schema.
pub fn assert_schema_path<P: AsRef<Path>>(path: P, schema_name: &str) {
    let text = fs::read_to_string(path.as_ref())
        .unwrap_or_else(|e| panic!("failed to read {:?}: {e}", path.as_ref()));
    assert_schema(&text, schema_name);
}
