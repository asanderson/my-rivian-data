//! WASM exports deliberately accept only non-secret local catalog and schema data.
//! There are no HTTP, WebSocket, credential, token, key, or command execution exports.

use rivian_core::parallax::{ParallaxError, TelemetryState, UpdateOutcome};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

const MAX_FIXTURE_JSON_BYTES: usize = 64 * 1024;
const MAX_FIXTURE_FRAMES: usize = 64;

#[wasm_bindgen]
pub fn catalog_json() -> String {
    // Every catalog value is serializable; retaining a valid JSON fallback avoids a trap.
    serde_json::to_string(&rivian_core::catalog()).unwrap_or_else(|_| "[]".into())
}

#[wasm_bindgen]
pub fn validate_request_json(operation_id: &str, variables_json: &str) -> String {
    let result = rivian_core::validate_json_request(operation_id, variables_json);
    serde_json::to_string(&result).unwrap_or_else(|_| {
        r#"{"valid":false,"issues":[{"field":"variables","message":"Validation unavailable"}]}"#
            .into()
    })
}

#[wasm_bindgen]
pub fn live_catalog_json() -> String {
    serde_json::to_string(&rivian_core::live::catalog()).unwrap_or_else(|_| "[]".into())
}

#[wasm_bindgen]
pub fn validate_live_request_json(operation_id: &str, variables_json: &str) -> String {
    serde_json::to_string(&rivian_core::live::validate_json_request(
        operation_id,
        variables_json,
    ))
    .unwrap_or_else(|_| {
        r#"{"valid":false,"issues":[{"field":"variables","message":"Validation unavailable"}]}"#
            .into()
    })
}

#[wasm_bindgen]
pub fn inventory_json() -> String {
    rivian_core::inventory::snapshot().to_string()
}

#[wasm_bindgen]
pub fn enriched_live_catalog_json() -> String {
    json!(rivian_core::inventory::enrich_catalog(
        &rivian_core::live::catalog()
    ))
    .to_string()
}

/// Pure local decoding of synthetic/non-secret fixtures. This does not connect
/// to a vehicle, validate ownership, or create native telemetry authority.
/// Input: {vehicle_id, messages: [{rvm, payload, timestamp}]} with no extra keys.
/// Envelope errors produce no state; rejected frames preserve accepted readings.
#[wasm_bindgen]
pub fn decode_parallax_fixture_json(input_json: &str) -> String {
    decode_fixture(input_json).to_string()
}

fn fixture_rejection(error: &str) -> Value {
    json!({"valid":false,"error":error,"outcomes":[],"snapshot":null,"developer_entries":[]})
}

fn error_code(error: ParallaxError) -> &'static str {
    match error {
        ParallaxError::InvalidVehicle => "invalid_vehicle",
        ParallaxError::InvalidTopic => "invalid_topic",
        ParallaxError::InvalidTimestamp => "invalid_timestamp",
        ParallaxError::PayloadTooLarge => "payload_too_large",
        ParallaxError::MalformedPayload => "malformed_payload",
        ParallaxError::InvalidReading => "invalid_reading",
        ParallaxError::StaleMessage => "stale_message",
        ParallaxError::ResourceLimit => "resource_limit",
    }
}

fn decode_fixture(input_json: &str) -> Value {
    if input_json.len() > MAX_FIXTURE_JSON_BYTES {
        return fixture_rejection("input_too_large");
    }
    let input: Value = match serde_json::from_str(input_json) {
        Ok(input) => input,
        Err(_) => return fixture_rejection("malformed_json"),
    };
    let Some(object) = input.as_object() else {
        return fixture_rejection("invalid_input");
    };
    let (Some(vehicle_id), Some(messages)) =
        (input["vehicle_id"].as_str(), input["messages"].as_array())
    else {
        return fixture_rejection("invalid_input");
    };
    if object.len() != 2 {
        return fixture_rejection("invalid_input");
    }
    if messages.len() > MAX_FIXTURE_FRAMES {
        return fixture_rejection("too_many_messages");
    }
    // Validate the whole envelope before creating/applying any state, so a
    // malformed later frame cannot accidentally publish a partial fixture.
    let frames: Option<Vec<_>> = messages
        .iter()
        .map(|message| {
            let object = message.as_object()?;
            if object.len() != 3 {
                return None;
            }
            Some((
                message["rvm"].as_str()?,
                message["payload"].as_str()?,
                message["timestamp"].as_i64()?,
            ))
        })
        .collect();
    let Some(frames) = frames else {
        return fixture_rejection("invalid_input");
    };
    let mut state = match TelemetryState::new(vehicle_id) {
        Ok(state) => state,
        Err(error) => return fixture_rejection(error_code(error)),
    };
    let outcomes: Vec<_> = frames
        .iter()
        .enumerate()
        .map(
            |(index, (rvm, payload, timestamp))| match state.apply(rvm, payload, *timestamp) {
                Ok(outcome) => json!({"index":index,"outcome":match outcome {
                    UpdateOutcome::Applied => "applied",
                    UpdateOutcome::Empty => "empty",
                    UpdateOutcome::Unknown => "unknown",
                }}),
                Err(error) => json!({"index":index,"error":error_code(error)}),
            },
        )
        .collect();
    json!({"valid":outcomes.iter().all(|item| item.get("error").is_none()),
        "error":null,"outcomes":outcomes,"snapshot":state.snapshot(),
        "developer_entries":state.developer_entries()})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_and_native_validation_match() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/validation-cases.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let operation = case["operation_id"].as_str().unwrap();
            let variables = case["variables_json"].as_str().unwrap();
            let wrapper: serde_json::Value =
                serde_json::from_str(&validate_request_json(operation, variables)).unwrap();
            assert_eq!(
                wrapper,
                serde_json::to_value(rivian_core::validate_json_request(operation, variables))
                    .unwrap()
            );
        }
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&catalog_json()).unwrap(),
            serde_json::to_value(rivian_core::catalog()).unwrap()
        );
    }

    #[test]
    fn live_wrapper_and_native_validation_match() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/live-validation-cases.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let operation = case["operation_id"].as_str().unwrap();
            let variables = case["variables_json"].as_str().unwrap();
            let wrapper: serde_json::Value =
                serde_json::from_str(&validate_live_request_json(operation, variables)).unwrap();
            assert_eq!(
                wrapper,
                serde_json::to_value(rivian_core::live::validate_json_request(
                    operation, variables
                ))
                .unwrap()
            );
        }
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&live_catalog_json()).unwrap(),
            serde_json::to_value(rivian_core::live::catalog()).unwrap()
        );
    }

    fn decode(input: serde_json::Value) -> serde_json::Value {
        serde_json::from_str(&decode_parallax_fixture_json(&input.to_string())).unwrap()
    }

    #[test]
    fn reviewed_inventory_and_enriched_catalog_match_native_core() {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&inventory_json()).unwrap(),
            rivian_core::inventory::snapshot()
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&enriched_live_catalog_json()).unwrap(),
            serde_json::to_value(rivian_core::inventory::enrich_catalog(
                &rivian_core::live::catalog()
            ))
            .unwrap()
        );
    }

    #[test]
    fn documented_fixture_has_native_readings_and_original_ages() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/parallax-readings.json")).unwrap();
        let result = decode(serde_json::json!({
            "vehicle_id":"fixture-vehicle", "messages":fixture["messages"]
        }));
        assert_eq!(result["valid"], true);
        for (field, expected) in fixture["expected"].as_object().unwrap() {
            assert_eq!(&result["snapshot"][field], expected, "{field}");
        }
        assert_eq!(
            result["snapshot"]["field_observed_at"]["battery_percent"],
            "2026-10-14T12:00:00Z"
        );
        assert_eq!(result["outcomes"].as_array().unwrap().len(), 11);
        assert_eq!(result["developer_entries"], serde_json::json!([]));
    }

    #[test]
    fn fixture_limits_cover_exact_byte_and_frame_boundaries() {
        let input = r#"{"vehicle_id":"fixture-vehicle","messages":[]}"#;
        let exact = format!("{input}{}", " ".repeat(64 * 1024 - input.len()));
        let result: serde_json::Value =
            serde_json::from_str(&decode_parallax_fixture_json(&exact)).unwrap();
        assert_eq!(result["valid"], true);
        let oversized: serde_json::Value =
            serde_json::from_str(&decode_parallax_fixture_json(&(exact + " "))).unwrap();
        assert_eq!(oversized["error"], "input_too_large");

        let messages: Vec<_> = (0..64)
            .map(|index| serde_json::json!({"rvm":"dynamics.vehicle.range", "payload":"", "timestamp":index}))
            .collect();
        assert_eq!(
            decode(serde_json::json!({"vehicle_id":"fixture-vehicle", "messages":messages}))["valid"],
            true
        );
        let messages = vec![
            serde_json::json!({"rvm":"dynamics.vehicle.range", "payload":"", "timestamp":0});
            65
        ];
        let result =
            decode(serde_json::json!({"vehicle_id":"fixture-vehicle", "messages":messages}));
        assert_eq!(result["error"], "too_many_messages");
        assert!(result["snapshot"].is_null());
    }

    #[test]
    fn malformed_envelopes_never_publish_partial_readings() {
        for input in [
            serde_json::json!({"vehicle_id":"fixture-vehicle", "messages":[], "password":"sample"}),
            serde_json::json!({"vehicle_id":"fixture-vehicle", "messages":[{"rvm":"dynamics.vehicle.range", "payload":"CLsC", "timestamp":1},{"rvm":"dynamics.vehicle.range", "payload":"", "timestamp":2.5}]}),
            serde_json::json!({"vehicle_id":"fixture-vehicle", "messages":[{"rvm":"dynamics.vehicle.range", "payload":"", "timestamp":1, "token":"sample"}]}),
        ] {
            let result = decode(input);
            assert_eq!(result["error"], "invalid_input");
            assert_eq!(result["outcomes"], serde_json::json!([]));
            assert!(result["snapshot"].is_null());
        }
        let result = decode(serde_json::json!({"vehicle_id":"../vehicle", "messages":[]}));
        assert_eq!(result["error"], "invalid_vehicle");
    }

    #[test]
    fn rejected_frame_does_not_advance_topic_or_owner_reading() {
        let result = decode(
            serde_json::json!({"vehicle_id":"fixture-vehicle", "messages":[
                {"rvm":"dynamics.vehicle.range", "payload":"CLsC", "timestamp":1},
                {"rvm":"dynamics.vehicle.range", "payload":"%%%", "timestamp":2},
                {"rvm":"dynamics.vehicle.range", "payload":"CAA=", "timestamp":2},
                {"rvm":"dynamics.vehicle.range", "payload":"CLsC", "timestamp":2}
            ]}),
        );
        assert_eq!(result["valid"], false);
        assert_eq!(result["outcomes"][1]["error"], "malformed_payload");
        assert_eq!(result["outcomes"][2]["outcome"], "applied");
        assert_eq!(result["outcomes"][3]["error"], "stale_message");
        assert_eq!(result["snapshot"]["estimated_range_km"], 0.0);
        assert_eq!(
            result["snapshot"]["field_observed_at"]["estimated_range_km"],
            "1970-01-01T00:00:00.002Z"
        );
    }
}
