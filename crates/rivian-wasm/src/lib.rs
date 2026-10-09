//! WASM exports deliberately accept only non-secret local catalog and schema data.
//! There are no HTTP, WebSocket, credential, token, key, or command execution exports.

use wasm_bindgen::prelude::*;

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
}
