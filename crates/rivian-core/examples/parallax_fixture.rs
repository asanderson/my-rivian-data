//! Native, credential-free reference results for compiled WASM fixture parity.
//! Envelope rejection cases have explicit expectations; reading cases call only
//! the portable core decoder, independently of the WASM adapter.
use base64::{Engine, engine::general_purpose::STANDARD};
use rivian_core::parallax::{ParallaxError, TelemetryState, UpdateOutcome};
use serde_json::{Value, json};

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

fn rejected(error: &str) -> Value {
    json!({"valid":false,"error":error,"outcomes":[],"snapshot":null,"developer_entries":[]})
}

fn native(input: &Value) -> Value {
    let mut state = match TelemetryState::new(input["vehicle_id"].as_str().unwrap()) {
        Ok(state) => state,
        Err(error) => return rejected(error_code(error)),
    };
    let outcomes: Vec<_> = input["messages"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, frame)| {
            match state.apply(
                frame["rvm"].as_str().unwrap(),
                frame["payload"].as_str().unwrap(),
                frame["timestamp"].as_i64().unwrap(),
            ) {
                Ok(outcome) => json!({"index":index,"outcome":match outcome {
                    UpdateOutcome::Applied => "applied",
                    UpdateOutcome::Empty => "empty",
                    UpdateOutcome::Unknown => "unknown",
                }}),
                Err(error) => json!({"index":index,"error":error_code(error)}),
            }
        })
        .collect();
    json!({"valid":outcomes.iter().all(|item| item.get("error").is_none()),
        "error":null,"outcomes":outcomes,"snapshot":state.snapshot(),
        "developer_entries":state.developer_entries()})
}

fn frame(rvm: &str, payload: &str, timestamp: i64) -> Value {
    json!({"rvm":rvm,"payload":payload,"timestamp":timestamp})
}

fn input(messages: Vec<Value>) -> Value {
    json!({"vehicle_id":"fixture-vehicle","messages":messages})
}

fn case(name: &str, input: Value) -> Value {
    json!({"name":name,"input_json":input.to_string(),"result":native(&input)})
}

fn envelope_case(name: &str, input_json: String, error: &str) -> Value {
    json!({"name":name,"input_json":input_json,"result":rejected(error)})
}

fn main() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/parallax-readings.json")).unwrap();
    let documented = fixture["messages"].as_array().unwrap().clone();
    let range = "dynamics.vehicle.range";
    let mut empty_boundary = case("exact-json-byte-limit", input(vec![]));
    let boundary = empty_boundary["input_json"].as_str().unwrap().to_owned();
    let boundary = format!("{boundary}{}", " ".repeat(64 * 1024 - boundary.len()));
    empty_boundary["input_json"] = json!(boundary);
    let mut unknowns: Vec<_> = (0..54)
        .map(|index| frame(&format!("unknown.topic_{index}"), "CAE=", 100))
        .collect();
    unknowns.push(frame(range, "CLsC", 100));
    let blob = STANDARD.encode([vec![0x7a, 0x80, 0x10], vec![0; 2048]].concat());
    let mut preserved = documented.clone();
    preserved.push(frame(
        "energy.high_voltage.battery_state",
        "",
        1791979300000,
    ));
    let cases = vec![
        case("documented-owner-readings", input(documented)),
        case("empty-state", input(vec![])),
        case("empty-frame-preserves-original-age", input(preserved)),
        case("explicit-zero", input(vec![frame(range, "CAA=", 100)])),
        case("exact-frame-limit", input((0..64).map(|index| frame(range, "", index)).collect())),
        empty_boundary,
        envelope_case("oversized-json", format!("{boundary} "), "input_too_large"),
        envelope_case("too-many-frames", input(vec![frame(range, "", 100);65]).to_string(), "too_many_messages"),
        envelope_case("malformed-json", "{".into(), "malformed_json"),
        envelope_case("invalid-envelope", "[]".into(), "invalid_input"),
        envelope_case("credentials-field-rejected", r#"{"vehicle_id":"fixture-vehicle","messages":[],"password":"synthetic"}"#.into(), "invalid_input"),
        envelope_case("frame-extra-field-rejected", r#"{"vehicle_id":"fixture-vehicle","messages":[{"rvm":"dynamics.vehicle.range","payload":"","timestamp":100,"token":"synthetic"}]}"#.into(), "invalid_input"),
        envelope_case("non-integer-time-rejected", r#"{"vehicle_id":"fixture-vehicle","messages":[{"rvm":"dynamics.vehicle.range","payload":"","timestamp":100.5}]}"#.into(), "invalid_input"),
        case("invalid-vehicle", json!({"vehicle_id":"../vehicle","messages":[]})),
        case("invalid-topic", input(vec![frame("bad topic", "", 100)])),
        case("invalid-timestamp", input(vec![frame(range, "", -1)])),
        case("malformed-base64", input(vec![frame(range, "%%%", 100)])),
        case("malformed-protobuf", input(vec![frame(range, "CA==", 100)])),
        case("published-invalid-battery-sample", input(vec![frame("energy.high_voltage.battery_state", "CkIKBAAAADMzT0ARAAAAQOESYEASDw00MytCFWdmOkIdzcwUQhoAIAEwCQ==", 100)])),
        case("invalid-reading", input(vec![frame(range, &STANDARD.encode([8, 0x89, 0x27]), 100)])),
        case("oversized-payload", input(vec![frame(range, &"A".repeat(rivian_core::parallax::MAX_BASE64_BYTES + 1), 100)])),
        case("stale-message", input(vec![frame(range, "CLsC", 100), frame(range, "CAA=", 100)])),
        case("rejected-frame-does-not-advance-time", input(vec![frame(range, "CLsC", 100), frame(range, "%%%", 101), frame(range, "CAA=", 101)])),
        case("unknown-topic-resource-limit-preserves-known-topic", input(unknowns)),
        case("unknown-developer-payload-is-truncated", input(vec![frame("unknown.blob", &blob, 100)])),
    ];
    println!(
        "{}",
        json!({"inventory":rivian_core::inventory::snapshot(),
        "enriched_live_catalog":rivian_core::inventory::enrich_catalog(&rivian_core::live::catalog()),
        "cases":cases})
    );
}
