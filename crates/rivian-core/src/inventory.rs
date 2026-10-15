//! Versioned reviewed source inventory, without native credentials or transport.
//!
//! Inventory membership never grants execution. `live::request` selects only
//! fixed reviewed local documents and the native host separately grants authority.
use std::sync::OnceLock;

use serde_json::{Value, json};

use crate::Operation;

const SNAPSHOT: &str = include_str!("../data/operations-2026-10-14.json");
static REVIEW: OnceLock<Value> = OnceLock::new();

fn review() -> &'static Value {
    REVIEW.get_or_init(|| serde_json::from_str(SNAPSHOT).expect("checked-in reviewed inventory"))
}

pub fn snapshot() -> Value {
    review().clone()
}

pub(crate) fn reviewed_document(name: &str) -> Option<&'static str> {
    review()["entries"]
        .as_array()?
        .iter()
        .find(|entry| entry["category"] == "graphql_document" && entry["operation_name"] == name)?
        .get("graphql_document")?
        .as_str()
}

/// Extra entries remain documented or blocked. They cannot select a network
/// destination or executable document through catalog membership.
pub(crate) fn known_catalog() -> Vec<Operation> {
    review()["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            if entry["local_operation_ids"]
                .as_array()
                .is_some_and(|ids| !ids.is_empty())
            {
                return None;
            }
            let source_url = entry["source_urls"].as_array()?.first()?.as_str()?;
            let kind = entry["kind"].as_str()?;
            Some(Operation {
                id: entry["id"].as_str()?.into(),
                label: entry["operation_name"].as_str()?.into(),
                family: entry["family"].as_str()?.into(),
                kind: if kind == "command" { "mutation" } else { kind }.into(),
                status: if matches!(kind, "mutation" | "command" | "subscription")
                    || entry["execution_status"] == "blocked"
                {
                    "blocked"
                } else {
                    "documented"
                }
                .into(),
                requires_vehicle: false,
                description: entry["description"].as_str()?.into(),
                example_variables: json!({}),
                source_url: source_url.into(),
            })
        })
        .collect()
}

/// Serialize shared Operation values with reviewed metadata without enlarging the
/// cross-mode model. The embedded exact execution document always comes from the
/// same fixed request constructor used natively.
pub fn enrich_catalog(operations: &[Operation]) -> Vec<Value> {
    operations.iter().map(|operation| {
        let mut result = serde_json::to_value(operation).expect("catalog serializes");
        let matching = review()["entries"].as_array().into_iter().flatten().find(|entry| {
            entry["id"] == operation.id || entry["local_operation_ids"].as_array().is_some_and(|ids| ids.iter().any(|id| id == &operation.id))
        });
        if let Some(entry) = matching {
            for field in ["operation_name", "category", "endpoint", "graphql_document", "document_fingerprint_sha256", "schema", "rvm", "source_urls", "reviewed_at", "source_version", "implementation_status", "verification_status", "last_tested_firmware", "last_live_tested_at", "local_operation_ids", "prerequisites", "retry_policy", "block_reason", "stream_supported", "execution_scope", "contract_notes"] {
                result[field] = entry[field].clone();
            }
        } else {
            result["implementation_status"] = json!(if operation.status == "live" { "implemented" } else { "blocked" });
            result["verification_status"] = json!("fixture_verified_live_unverified");
            result["reviewed_at"] = json!("2026-10-14");
            result["last_tested_firmware"] = Value::Null;
            result["last_live_tested_at"] = Value::Null;
            result["source_urls"] = json!([operation.source_url]);
            result["source_version"] = json!({"local_review":"2026-10-14","inherited_contract_review":"2026-10-09","python_client_commit":"4d15dd88e74cf1a0be0bd23f46565fe89b48af44"});
        }
        if operation.status == "live"
            && let Ok(request) = crate::live::request(&operation.id, &operation.example_variables, None)
        {
                result["operation_name"] = json!(request.operation_name);
                result["endpoint"] = json!(request.endpoint.url());
                // A source document can use a larger selection set. This field
                // describes the exact admitted execution contract instead.
                result["graphql_document"] = json!(request.query);
                result["upstream_schema"] = result["schema"].clone();
                result["schema"] = variable_schema(operation);
                result["source_document_fingerprint_sha256"] = result["document_fingerprint_sha256"].clone();
                result["document_fingerprint_sha256"] = review()["local_documents"][&operation.id]["fingerprint_sha256"].clone();
                result["implementation_status"] = json!("implemented");
                result["verification_status"] = json!("fixture_verified_live_unverified");
                result["requires_confirmation"] = json!(crate::live::requires_confirmation(&operation.id));
                result["resource_id_field"] = json!(crate::live::resource_id_field(&operation.id));
                result["retry_policy"] = json!(if operation.kind == "mutation" { "Never automatically retry; native one-use confirmation expires after two minutes" } else { "At most one native retry after app-session renewal; invalid user session requires sign-in" });
                result["block_reason"] = Value::Null;
        }
        if operation.id == "telemetry-subscription" {
            result["transport"] = json!("native_websocket");
            result["stream_supported"] = json!(true);
            result["stream_topics"] = json!(crate::parallax::READ_RVMS);
            result["schema"] = variable_schema(operation);
        } else if result["endpoint"].as_str().is_some_and(|url| url.starts_with("wss://")) {
            result["transport"] = json!("native_websocket");
        } else {
            result["transport"] = json!("fixed_https_graphql");
        }
        result
    }).collect()
}

fn variable_schema(operation: &Operation) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    let resource = crate::live::resource_id_field(&operation.id);
    if let Some(field) = resource {
        properties.insert(field.into(), json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_-]+$","description":"Syntax validation only; native account ownership is checked separately"}));
        required.push(field);
    }
    match operation.id.as_str() {
        "charging-history" => {
            properties.insert(
                "limit".into(),
                json!({"type":"integer","minimum":1,"maximum":100,"default":100}),
            );
        }
        "rename-vehicle" => {
            properties.insert("name".into(), json!({"type":"string","minLength":1,"maxLength":64,"maxUtf8Bytes":64,"description":"At least one non-whitespace character; control characters are rejected"}));
            required.push("name");
        }
        "charging-sites" => {
            for (field, min, max) in [("latitude", -90, 90), ("longitude", -180, 180)] {
                properties.insert(
                    field.into(),
                    json!({"type":"number","minimum":min,"maximum":max}),
                );
                required.push(field);
            }
            properties.insert(
                "connector_type".into(),
                json!({"type":"string","enum":["CCS","NACS"]}),
            );
            properties.insert(
                "max_results".into(),
                json!({"type":"integer","minimum":1,"maximum":100,"default":100}),
            );
            required.push("connector_type");
        }
        _ => {}
    }
    json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn source_snapshot_records_the_complete_known_union_and_honest_provenance() {
        let snapshot = snapshot();
        assert_eq!(snapshot["schema_version"], 1);
        assert_eq!(snapshot["snapshot_version"], "2026-10-14.1");
        let entries = snapshot["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 234);
        let mut ids = HashSet::new();
        for entry in entries {
            assert!(ids.insert(entry["id"].as_str().unwrap()));
            assert!(entry["last_tested_firmware"].is_null());
            assert!(entry["last_live_tested_at"].is_null());
            assert_eq!(entry["reviewed_at"], "2026-10-14");
            assert!(
                entry["source_urls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|source| source.as_str().unwrap().contains("/blob/"))
            );
            assert!(!entry["prerequisites"].as_array().unwrap().is_empty());
            assert!(entry["retry_policy"].is_string());
        }
        for (category, count) in [
            ("graphql_document", 69),
            ("schema_field", 44),
            ("parallax_rvm", 75),
            ("legacy_command", 46),
        ] {
            assert_eq!(
                entries
                    .iter()
                    .filter(|entry| entry["category"] == category)
                    .count(),
                count
            );
        }
    }

    #[test]
    fn executable_metadata_exposes_bounded_local_schema_and_fixed_document() {
        let catalog = enrich_catalog(&crate::live::catalog());
        let rename = catalog
            .iter()
            .find(|entry| entry["id"] == "rename-vehicle")
            .unwrap();
        assert_eq!(rename["implementation_status"], "implemented");
        assert_eq!(rename["requires_confirmation"], true);
        assert_eq!(rename["resource_id_field"], "vehicle_id");
        assert_eq!(rename["schema"]["additionalProperties"], false);
        assert_eq!(rename["schema"]["properties"]["name"]["maxLength"], 64);
        assert!(
            rename["graphql_document"]
                .as_str()
                .unwrap()
                .starts_with("mutation SetVehicleName")
        );
        let map = catalog
            .iter()
            .find(|entry| entry["id"] == "charging-sites")
            .unwrap();
        assert_eq!(map["schema"]["properties"]["latitude"]["minimum"], -90);
        assert_eq!(map["schema"]["properties"]["max_results"]["maximum"], 100);
    }

    #[test]
    fn local_fingerprints_are_bound_to_the_actual_fixed_execution_documents() {
        for operation in crate::live::catalog()
            .into_iter()
            .filter(|op| op.status == "live")
        {
            let request =
                crate::live::request(&operation.id, &operation.example_variables, None).unwrap();
            let recorded = &review()["local_documents"][&operation.id];
            assert_eq!(recorded["query"], request.query, "{}", operation.id);
            assert_eq!(recorded["endpoint"], request.endpoint.url());
            assert_eq!(recorded["operation_name"], request.operation_name);
            let hash = recorded["fingerprint_sha256"].as_str().unwrap();
            assert_eq!(hash.len(), 64);
            assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
        }
        for secret_doc in [
            "Login",
            "LoginWithOTP",
            "CreateCSRFToken",
            "CurrentUserForLogin",
        ] {
            assert!(reviewed_document(secret_doc).is_none());
        }
    }
}
