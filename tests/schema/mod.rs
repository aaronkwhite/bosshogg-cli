use crate::common::TestHarness;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn schema_hogql_dumps_tables_and_columns() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .and(body_partial_json(json!({
            "query": { "kind": "DatabaseSchemaQuery" }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tables": {
                "events": {
                    "type": "posthog",
                    "id": "events",
                    "name": "events",
                    "fields": {
                        "uuid": {"name": "uuid", "hogql_value": "uuid", "type": "string", "schema_valid": true},
                        "timestamp": {"name": "timestamp", "hogql_value": "timestamp", "type": "datetime", "schema_valid": true}
                    }
                },
                "orders": {
                    "type": "data_warehouse",
                    "id": "orders",
                    "name": "orders",
                    "fields": {
                        "id": {"name": "id", "hogql_value": "id", "type": "string", "schema_valid": true}
                    }
                },
                "numbers": {
                    "type": "system",
                    "id": "numbers",
                    "name": "numbers",
                    "fields": {}
                }
            },
            "joins": []
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["schema", "hogql", "--json"])
        .assert()
        .success()
        .stdout(contains("\"tables\""))
        .stdout(contains("\"events\""))
        .stdout(contains("\"orders\""))
        // system tables are filtered out
        .stdout(contains("numbers").not())
        .stdout(contains("\"datetime\""))
        .stdout(contains("\"warehouse_tables\""));
}
