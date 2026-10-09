//! Emit deterministic native results for comparison against the compiled WASM module.
//! Usage: cargo run -q -p rivian-core --example validation_fixture
use serde_json::{Value, json};

fn main() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/validation-cases.json"))
            .expect("checked-in fixture must be valid JSON");
    let cases: Vec<Value> = fixture["cases"]
        .as_array()
        .expect("fixture cases")
        .iter()
        .map(|case| {
            let result = rivian_core::validate_json_request(
                case["operation_id"].as_str().expect("operation id"),
                case["variables_json"].as_str().expect("JSON input"),
            );
            json!({"name": case["name"], "result": result})
        })
        .collect();
    let live_fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/live-validation-cases.json"))
            .expect("live fixture JSON");
    let live_cases: Vec<Value> = live_fixture["cases"]
        .as_array()
        .expect("live cases")
        .iter()
        .map(|case| {
            let result = rivian_core::live::validate_json_request(
                case["operation_id"].as_str().expect("operation id"),
                case["variables_json"].as_str().expect("JSON input"),
            );
            json!({"name": case["name"], "result": result})
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string(&json!({"cases": cases, "catalog": rivian_core::catalog(), "live_cases": live_cases, "live_catalog": rivian_core::live::catalog()})).unwrap()
    );
}
