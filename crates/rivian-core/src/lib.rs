//! A deliberately offline, deterministic prototype core.
//!
//! Catalog IDs are local adapter names, not claims about Rivian's upstream API.
//! This crate performs no I/O, handles no credentials, and sends no commands.
//! Browser/WASM validation is an aid; native validation remains authoritative.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

pub const MAX_VARIABLE_BYTES: usize = 16 * 1024;
pub const MAX_VARIABLE_DEPTH: usize = 8;
pub const MAX_VARIABLE_NODES: usize = 256;
pub const DEMO_SOURCE: &str = "synthetic-demo";
pub const DEMO_OBSERVED_AT: &str = "2026-01-01T12:00:00Z";
const DEMO_VEHICLE_IDS: [&str; 2] = ["demo-r1t-001", "demo-r1s-002"];
const SOURCE_URL: &str = "https://rivian-api.kaedenb.org/app/";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Operation {
    pub id: String,
    pub label: String,
    pub family: String,
    pub kind: String,
    pub description: String,
    pub status: String,
    pub requires_vehicle: bool,
    pub example_variables: Value,
    pub source_url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_m: f64,
}

/// Normalized telemetry. All values and coordinates in this prototype are fictional.
/// A timestamp is fixed fixture time; it never represents live vehicle freshness.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Vehicle {
    pub id: String,
    pub name: String,
    pub model: String,
    pub model_year: u16,
    pub battery_percent: f64,
    pub estimated_range_km: f64,
    pub odometer_km: f64,
    pub locked: bool,
    pub charging_state: String,
    pub temperature_celsius: f64,
    pub location: Location,
    pub observed_at: String,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ValidationIssue {
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ValidationResult {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationResult {
    fn from_issues(issues: Vec<ValidationIssue>) -> Self {
        Self {
            valid: issues.is_empty(),
            issues,
        }
    }

    fn reject(field: &str, message: &str) -> Self {
        Self::from_issues(vec![issue(field, message)])
    }
}

/// Errors use static messages, so submitted JSON or identifiers cannot leak via logs.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Unknown local operation")]
    UnknownOperation,
    #[error("Operation is disabled in the offline prototype")]
    OperationDisabled,
    #[error("Request variables failed validation")]
    InvalidRequest,
}

fn issue(field: &str, message: &str) -> ValidationIssue {
    ValidationIssue {
        field: field.to_owned(),
        message: message.to_owned(),
    }
}

/// A representative catalog, not a complete inventory of Rivian operations.
pub fn catalog() -> Vec<Operation> {
    let definitions = [
        (
            "account-summary",
            "Demo account",
            "Account",
            "query",
            "demo",
            false,
            "Show a synthetic account summary; no sign-in or Rivian account lookup occurs.",
            json!({}),
        ),
        (
            "list-vehicles",
            "List demo vehicles",
            "Vehicles",
            "query",
            "demo",
            false,
            "Return two fictional vehicles with normalized units and fixed fixture timestamps.",
            json!({}),
        ),
        (
            "vehicle-state",
            "Demo vehicle state",
            "Telemetry",
            "query",
            "demo",
            true,
            "Return synthetic battery, range, lock state, and other telemetry.",
            json!({"vehicle_id": DEMO_VEHICLE_IDS[0]}),
        ),
        (
            "charging-status",
            "Demo charging status",
            "Charging",
            "query",
            "demo",
            true,
            "Return synthetic charging state and a sample charge limit.",
            json!({"vehicle_id": DEMO_VEHICLE_IDS[0]}),
        ),
        (
            "vehicle-location",
            "Demo location",
            "Location",
            "query",
            "demo",
            true,
            "Return fictional coordinates from a fixed fixture; never a real vehicle location.",
            json!({"vehicle_id": DEMO_VEHICLE_IDS[0]}),
        ),
        (
            "charging-history",
            "Demo charging history",
            "Charging",
            "query",
            "demo",
            true,
            "Return a bounded page of synthetic sessions. Optional limit is an integer from 1 to 100.",
            json!({"vehicle_id": DEMO_VEHICLE_IDS[0], "limit": 10}),
        ),
        (
            "telemetry-subscription",
            "Live telemetry subscription",
            "Telemetry",
            "subscription",
            "planned",
            true,
            "Planned only. Authenticated native transport and reconnect handling are not implemented.",
            json!({"vehicle_id": DEMO_VEHICLE_IDS[0]}),
        ),
        (
            "lock-vehicle",
            "Lock vehicle",
            "Commands",
            "mutation",
            "blocked",
            true,
            "Blocked. This prototype has no vehicle command transport or signing authority.",
            json!({"vehicle_id": DEMO_VEHICLE_IDS[0]}),
        ),
        (
            "unlock-vehicle",
            "Unlock vehicle",
            "Commands",
            "mutation",
            "blocked",
            true,
            "Blocked. This prototype cannot unlock a vehicle.",
            json!({"vehicle_id": DEMO_VEHICLE_IDS[0]}),
        ),
        (
            "set-charge-limit",
            "Set charge limit",
            "Commands",
            "mutation",
            "blocked",
            true,
            "Blocked. A sample schema demonstrates validation; no setting can be changed.",
            json!({"vehicle_id": DEMO_VEHICLE_IDS[0], "limit_percent": 80}),
        ),
    ];
    definitions
        .into_iter()
        .map(
            |(
                id,
                label,
                family,
                kind,
                status,
                requires_vehicle,
                description,
                example_variables,
            )| Operation {
                id: id.into(),
                label: label.into(),
                family: family.into(),
                kind: kind.into(),
                description: description.into(),
                status: status.into(),
                requires_vehicle,
                example_variables,
                source_url: if id == "telemetry-subscription" {
                    "https://rivian-api.kaedenb.org/app/parallax/".into()
                } else {
                    SOURCE_URL.into()
                },
            },
        )
        .collect()
}

/// Validate JSON before deserializing or transporting it. Oversized requests are rejected
/// before parse. serde_json's own recursion limit also bounds malicious parser input.
pub fn validate_json_request(operation_id: &str, variables_json: &str) -> ValidationResult {
    if variables_json.len() > MAX_VARIABLE_BYTES {
        return ValidationResult::reject("variables", "Variables exceed the 16 KiB limit");
    }
    let variables = match serde_json::from_str::<Value>(variables_json) {
        Ok(value) => value,
        Err(_) => return ValidationResult::reject("variables", "Variables must be valid JSON"),
    };
    validate_request(operation_id, &variables)
}

/// Native-side validation must run on every execution, even if the browser has validated.
pub fn validate_request(operation_id: &str, variables: &Value) -> ValidationResult {
    // Check structural limits before serializing an already parsed Value.
    if let Some(message) = structure_issue(variables) {
        return ValidationResult::reject("variables", message);
    }
    match serde_json::to_vec(variables) {
        Ok(bytes) if bytes.len() <= MAX_VARIABLE_BYTES => {}
        _ => return ValidationResult::reject("variables", "Variables exceed the 16 KiB limit"),
    }
    let Some(operation) = catalog()
        .into_iter()
        .find(|candidate| candidate.id == operation_id)
    else {
        return ValidationResult::reject("operation_id", "Unknown local operation");
    };
    let Some(object) = variables.as_object() else {
        return ValidationResult::reject("variables", "Variables must be a JSON object");
    };
    let mut issues = Vec::new();
    if operation.status != "demo" {
        issues.push(issue(
            "operation_id",
            "Operation is disabled in the offline prototype",
        ));
    }
    let allowed: &[&str] = match operation_id {
        "account-summary" | "list-vehicles" => &[],
        "charging-history" => &["vehicle_id", "limit"],
        "set-charge-limit" => &["vehicle_id", "limit_percent"],
        _ => &["vehicle_id"],
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        // Do not reflect attacker-controlled property names into a DOM or log.
        issues.push(issue(
            "variables",
            "Unknown variable fields are not allowed",
        ));
    }
    if operation.requires_vehicle {
        match object.get("vehicle_id") {
            None => issues.push(issue("vehicle_id", "Vehicle ID is required")),
            Some(Value::String(id)) if DEMO_VEHICLE_IDS.contains(&id.as_str()) => {}
            Some(Value::String(_)) => issues.push(issue(
                "vehicle_id",
                "Vehicle is not authorized for the demo account",
            )),
            Some(_) => issues.push(issue("vehicle_id", "Vehicle ID must be a string")),
        }
    }
    if object
        .get("limit")
        .is_some_and(|value| !value.as_u64().is_some_and(|n| (1..=100).contains(&n)))
    {
        issues.push(issue("limit", "Limit must be an integer from 1 to 100"));
    }
    if operation_id == "set-charge-limit"
        && !object
            .get("limit_percent")
            .and_then(Value::as_u64)
            .is_some_and(|n| (50..=100).contains(&n))
    {
        issues.push(issue(
            "limit_percent",
            "Charge limit must be an integer from 50 to 100",
        ));
    }
    ValidationResult::from_issues(issues)
}

fn structure_issue(value: &Value) -> Option<&'static str> {
    let mut pending = vec![(value, 1usize)];
    let mut count = 0usize;
    while let Some((value, depth)) = pending.pop() {
        count += 1;
        if depth > MAX_VARIABLE_DEPTH {
            return Some("Variables exceed the maximum depth of 8");
        }
        if count > MAX_VARIABLE_NODES {
            return Some("Variables exceed the maximum of 256 values");
        }
        match value {
            Value::Object(object) => pending.extend(object.values().map(|v| (v, depth + 1))),
            Value::Array(array) => pending.extend(array.iter().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    None
}

pub fn demo_vehicles() -> Vec<Vehicle> {
    vec![
        Vehicle {
            id: DEMO_VEHICLE_IDS[0].into(),
            name: "Demo R1T".into(),
            model: "R1T".into(),
            model_year: 2025,
            battery_percent: 72.0,
            estimated_range_km: 362.1,
            odometer_km: 12840.0,
            locked: true,
            charging_state: "disconnected".into(),
            temperature_celsius: 21.0,
            location: Location {
                latitude: 47.0,
                longitude: -122.0,
                accuracy_m: 25.0,
            },
            observed_at: DEMO_OBSERVED_AT.into(),
            source: DEMO_SOURCE.into(),
        },
        Vehicle {
            id: DEMO_VEHICLE_IDS[1].into(),
            name: "Demo R1S".into(),
            model: "R1S".into(),
            model_year: 2025,
            battery_percent: 48.0,
            estimated_range_km: 241.4,
            odometer_km: 8050.5,
            locked: true,
            charging_state: "charging".into(),
            temperature_celsius: 19.5,
            location: Location {
                latitude: 47.05,
                longitude: -122.05,
                accuracy_m: 25.0,
            },
            observed_at: DEMO_OBSERVED_AT.into(),
            source: DEMO_SOURCE.into(),
        },
    ]
}

/// Return deterministic synthetic data. No mutation or subscription is executable.
pub fn execute_demo(operation_id: &str, variables: &Value) -> Result<Value, CoreError> {
    let operation = catalog()
        .into_iter()
        .find(|candidate| candidate.id == operation_id)
        .ok_or(CoreError::UnknownOperation)?;
    if operation.kind != "query" || operation.status != "demo" {
        return Err(CoreError::OperationDisabled);
    }
    if !validate_request(operation_id, variables).valid {
        return Err(CoreError::InvalidRequest);
    }
    let data = match operation_id {
        "account-summary" => {
            json!({"id": "demo-account-001", "display_name": "Offline demo account", "vehicle_count": 2})
        }
        "list-vehicles" => json!({"vehicles": demo_vehicles()}),
        _ => {
            let vehicle = demo_vehicles()
                .into_iter()
                .find(|vehicle| {
                    Some(vehicle.id.as_str()) == variables.get("vehicle_id").and_then(Value::as_str)
                })
                .ok_or(CoreError::InvalidRequest)?;
            match operation_id {
                "vehicle-state" => json!({"vehicle": vehicle}),
                "charging-status" => {
                    json!({"vehicle_id": vehicle.id, "state": vehicle.charging_state,
                    "battery_percent": vehicle.battery_percent, "limit_percent": 80,
                    "power_kw": if vehicle.charging_state == "charging" { 7.2 } else { 0.0 }})
                }
                "vehicle-location" => {
                    json!({"vehicle_id": vehicle.id, "location": vehicle.location})
                }
                "charging-history" => {
                    let limit =
                        variables.get("limit").and_then(Value::as_u64).unwrap_or(10) as usize;
                    let sessions: Vec<Value> = vec![
                        json!({"id": "synthetic-session-001", "started_at": "2025-12-31T21:00:00Z", "energy_kwh": 18.5, "duration_minutes": 155}),
                        json!({"id": "synthetic-session-002", "started_at": "2025-12-30T18:30:00Z", "energy_kwh": 25.0, "duration_minutes": 210}),
                    ].into_iter().take(limit).collect();
                    json!({"vehicle_id": vehicle.id, "sessions": sessions, "has_more": limit < 2})
                }
                _ => return Err(CoreError::UnknownOperation),
            }
        }
    };
    Ok(json!({"operation_id": operation_id, "source": DEMO_SOURCE,
        "observed_at": DEMO_OBSERVED_AT, "synthetic": true, "data": data}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_examples_have_explicit_execution_status() {
        for operation in catalog() {
            let validation = validate_request(&operation.id, &operation.example_variables);
            assert_eq!(
                validation.valid,
                operation.status == "demo",
                "{}",
                operation.id
            );
            assert_eq!(
                execute_demo(&operation.id, &operation.example_variables).is_ok(),
                operation.status == "demo"
            );
        }
    }

    #[test]
    fn commands_never_execute_even_with_valid_fields() {
        for id in [
            "lock-vehicle",
            "unlock-vehicle",
            "set-charge-limit",
            "telemetry-subscription",
        ] {
            assert!(matches!(
                execute_demo(
                    id,
                    &json!({"vehicle_id": DEMO_VEHICLE_IDS[0], "limit_percent": 80})
                ),
                Err(CoreError::OperationDisabled)
            ));
        }
    }

    #[test]
    fn every_unknown_or_cross_account_vehicle_is_rejected() {
        for id in [
            "",
            "other-account-vehicle",
            "demo-r1t-001/../admin",
            "DEMO-R1T-001",
        ] {
            assert!(!validate_request("vehicle-state", &json!({"vehicle_id": id})).valid);
        }
    }

    #[test]
    fn execution_revalidates_without_trusting_browser_success() {
        for variables in [
            json!({"vehicle_id": "other-account-vehicle"}),
            json!({"vehicle_id": DEMO_VEHICLE_IDS[0], "unexpected": true}),
            json!({"vehicle_id": DEMO_VEHICLE_IDS[0], "limit": 101}),
        ] {
            assert!(matches!(
                execute_demo("charging-history", &variables),
                Err(CoreError::InvalidRequest)
            ));
        }
        assert!(matches!(
            execute_demo("unknown-operation", &json!({})),
            Err(CoreError::UnknownOperation)
        ));
    }

    #[test]
    fn strict_schemas_reject_coercion_and_extras() {
        for vars in [
            json!(null),
            json!([]),
            json!({"vehicle_id": 1}),
            json!({"vehicle_id": DEMO_VEHICLE_IDS[0], "url": "https://invalid.example"}),
        ] {
            assert!(!validate_request("vehicle-state", &vars).valid);
        }
        for limit in [
            json!("10"),
            json!(1.5),
            json!(-1),
            json!(0),
            json!(101),
            json!(true),
            json!(null),
        ] {
            assert!(
                !validate_request(
                    "charging-history",
                    &json!({"vehicle_id": DEMO_VEHICLE_IDS[0], "limit": limit})
                )
                .valid
            );
        }
        for limit in [1, 100] {
            assert!(
                validate_request(
                    "charging-history",
                    &json!({"vehicle_id": DEMO_VEHICLE_IDS[0], "limit": limit})
                )
                .valid
            );
        }
    }

    #[test]
    fn raw_size_depth_and_node_limits_are_enforced() {
        assert!(!validate_json_request("list-vehicles", &" ".repeat(MAX_VARIABLE_BYTES + 1)).valid);
        assert!(
            validate_json_request(
                "list-vehicles",
                &format!("{}{{}}", " ".repeat(MAX_VARIABLE_BYTES - 2))
            )
            .valid
        );
        let mut nested = json!(null);
        for _ in 0..MAX_VARIABLE_DEPTH {
            nested = json!({"nested": nested});
        }
        assert!(
            validate_request("list-vehicles", &nested).issues[0]
                .message
                .contains("depth")
        );
        let many = json!({"x": vec![0; MAX_VARIABLE_NODES]});
        assert!(
            validate_request("list-vehicles", &many).issues[0]
                .message
                .contains("256")
        );
        let too_big = json!({"x": "x".repeat(MAX_VARIABLE_BYTES)});
        assert!(
            validate_request("list-vehicles", &too_big).issues[0]
                .message
                .contains("16 KiB")
        );
    }

    #[test]
    fn errors_do_not_echo_untrusted_text() {
        let sentinel = "DO_NOT_REFLECT_TEST_INPUT";
        for result in [
            validate_json_request("list-vehicles", sentinel),
            validate_request(sentinel, &json!({})),
            validate_request("list-vehicles", &json!({sentinel: sentinel})),
        ] {
            assert!(!serde_json::to_string(&result).unwrap().contains(sentinel));
        }
    }

    #[test]
    fn all_demo_results_are_labeled_synthetic_and_stable() {
        for operation in catalog()
            .into_iter()
            .filter(|operation| operation.status == "demo")
        {
            let result = execute_demo(&operation.id, &operation.example_variables).unwrap();
            assert_eq!(result["source"], DEMO_SOURCE);
            assert_eq!(result["observed_at"], DEMO_OBSERVED_AT);
            assert_eq!(result["synthetic"], true);
            assert_eq!(
                result,
                execute_demo(&operation.id, &operation.example_variables).unwrap()
            );
        }
    }

    #[test]
    fn charging_history_respects_requested_limit() {
        let result = execute_demo(
            "charging-history",
            &json!({"vehicle_id": DEMO_VEHICLE_IDS[0], "limit": 1}),
        )
        .unwrap();
        assert_eq!(result["data"]["sessions"].as_array().unwrap().len(), 1);
        assert_eq!(result["data"]["has_more"], true);
    }

    #[test]
    fn sanitized_parity_fixture_expectations_hold() {
        let fixtures: Value =
            serde_json::from_str(include_str!("../../../fixtures/validation-cases.json")).unwrap();
        for case in fixtures["cases"].as_array().unwrap() {
            let result = validate_json_request(
                case["operation_id"].as_str().unwrap(),
                case["variables_json"].as_str().unwrap(),
            );
            assert_eq!(
                result.valid,
                case["expected_valid"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
        }
    }
}
