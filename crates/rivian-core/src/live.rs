//! Reviewed, fixed GraphQL documents. This module has no network or token access.
//!
//! Contracts were checked against RivDocs and rivian-python-client commit
//! 4d15dd88e74cf1a0be0bd23f46565fe89b48af44 on 2026-10-09. These are unofficial
//! contracts, not a claim that all Rivian operations or vehicles are supported.
//! Native transport must separately check that each requested vehicle belongs to
//! the authenticated account. Browser validation cannot grant that authority.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{CoreError, MAX_VARIABLE_BYTES, Operation, ValidationResult, issue, structure_issue};

pub const SOURCE: &str = "live";
const EXAMPLE_VEHICLE: &str = "select-a-vehicle";
const PYTHON_SOURCE: &str = "https://github.com/bretterer/rivian-python-client/blob/4d15dd88e74cf1a0be0bd23f46565fe89b48af44/src/rivian/rivian.py";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveEndpoint {
    Gateway,
    Charging,
    Orders,
}

impl LiveEndpoint {
    /// No caller-controlled hostname, path, redirect or URL can enter this map.
    pub const fn url(self) -> &'static str {
        match self {
            Self::Gateway => "https://rivian.com/api/gql/gateway/graphql",
            Self::Charging => "https://rivian.com/api/gql/chrg/user/graphql",
            Self::Orders => "https://rivian.com/api/gql/orders/graphql",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LiveRequest {
    pub endpoint: LiveEndpoint,
    pub operation_name: String,
    pub query: String,
    pub variables: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountIdentity {
    pub user_id: String,
    pub vehicle_ids: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NormalizeError {
    #[error("Rivian returned an incomplete or unexpected response")]
    InvalidResponse,
    #[error("The requested vehicle is not in the authenticated account")]
    VehicleNotOwned,
    #[error("Rivian reported an operation error")]
    UpstreamError,
    #[error("Unknown local operation")]
    UnknownOperation,
}

/// Live read operations and explicit unavailable capabilities. Every executable
/// entry has one fixed document; this is deliberately not a general GraphQL proxy.
pub fn catalog() -> Vec<Operation> {
    let definitions = [
        (
            "account-summary",
            "Account summary",
            "Account",
            false,
            "Read account identity and the number of vehicles available to this Rivian account.",
            "https://rivian-api.kaedenb.org/app/account/user-info/",
        ),
        (
            "list-vehicles",
            "Your vehicles",
            "Vehicles",
            false,
            "Read vehicles available to this account. Missing readings remain unavailable until requested.",
            "https://rivian-api.kaedenb.org/app/account/user-info/",
        ),
        (
            "vehicle-state",
            "Vehicle readings",
            "Telemetry",
            true,
            "Read battery charge, estimated range, odometer, door locks, cabin temperature and location. Readings can have different ages.",
            "https://rivian-api.kaedenb.org/app/legacy/vehicle-state/",
        ),
        (
            "charging-status",
            "Charging status",
            "Charging",
            true,
            "Read the reported charging state, battery charge and charge limit. Charging power is unavailable in this response.",
            "https://rivian-api.kaedenb.org/app/legacy/vehicle-state/",
        ),
        (
            "vehicle-location",
            "Vehicle location",
            "Location",
            true,
            "Read reported coordinates and their source timestamp. No map service receives these coordinates.",
            "https://rivian-api.kaedenb.org/app/legacy/vehicle-state/",
        ),
        (
            "charging-history",
            "Vehicle charging history",
            "Charging",
            true,
            "Read up to 100 completed sessions explicitly associated with this vehicle. Unassigned sessions are only shown in account charging history.",
            "https://rivian-api.kaedenb.org/app/legacy/charging/get-completed-session-summaries/",
        ),
        (
            "vehicle-telemetry",
            "Detailed vehicle readings",
            "Telemetry",
            true,
            "Read a fixed set of raw, timestamped vehicle telemetry including tire status, closures, climate and software status.",
            "https://rivian-api.kaedenb.org/app/legacy/vehicle-state/",
        ),
        (
            "live-charging-session",
            "Current charging session",
            "Charging",
            true,
            "Read the current charging session records, when Rivian provides them.",
            PYTHON_SOURCE,
        ),
        (
            "account-charging-history",
            "Account charging history",
            "Charging",
            false,
            "Read completed sessions for this account, including home sessions without an associated vehicle ID. These are not assigned to a vehicle.",
            "https://rivian-api.kaedenb.org/app/legacy/charging/get-completed-session-summaries/",
        ),
        (
            "charging-schedules",
            "Charging schedules",
            "Charging",
            true,
            "Read existing charging schedules. This operation cannot change them.",
            PYTHON_SOURCE,
        ),
        (
            "registered-wallboxes",
            "Your wall chargers",
            "Charging",
            false,
            "Read wall chargers registered to this account and their reported status.",
            PYTHON_SOURCE,
        ),
        (
            "supported-features",
            "Vehicle capabilities",
            "Vehicles",
            false,
            "Read the feature flags returned for each account vehicle.",
            "https://rivian-api.kaedenb.org/app/legacy/vehicle-info/supported-features/",
        ),
        (
            "vehicle-connection",
            "Last vehicle connection",
            "Telemetry",
            true,
            "Read the last cloud synchronization time reported by Rivian.",
            "https://rivian-api.kaedenb.org/app/legacy/vehicle-info/vehicle-last-connection/",
        ),
        (
            "ota-updates",
            "Software update details",
            "Vehicles",
            true,
            "Read current and available software release note metadata. No update is scheduled or installed.",
            PYTHON_SOURCE,
        ),
        (
            "vehicle-images",
            "Vehicle image metadata",
            "Vehicles",
            false,
            "Read image URLs and metadata for account vehicles and orders; this app does not fetch those external images.",
            PYTHON_SOURCE,
        ),
        (
            "drivers-and-keys",
            "Drivers and key metadata",
            "Vehicles",
            true,
            "Read invited drivers and device enrollment metadata. No key enrollment or command signing is performed.",
            PYTHON_SOURCE,
        ),
        (
            "account-profile",
            "Account profile",
            "Account",
            false,
            "Read your account name, contact information and vehicle role metadata.",
            "https://github.com/bretterer/rivian-python-client/blob/4d15dd88e74cf1a0be0bd23f46565fe89b48af44/src/rivian/schemas/orders.graphql",
        ),
        (
            "vehicle-orders",
            "Order summaries",
            "Account",
            false,
            "Read account order summaries including state, totals and vehicle association.",
            "https://github.com/bretterer/rivian-python-client/blob/4d15dd88e74cf1a0be0bd23f46565fe89b48af44/src/rivian/schemas/orders.graphql",
        ),
    ];
    let mut operations: Vec<Operation> = definitions
        .into_iter()
        .map(
            |(id, label, family, requires_vehicle, description, source_url)| Operation {
                id: id.into(),
                label: label.into(),
                family: family.into(),
                kind: "query".into(),
                status: "live".into(),
                requires_vehicle,
                description: description.into(),
                example_variables: if id == "charging-history" {
                    json!({"vehicle_id": EXAMPLE_VEHICLE, "limit": 100})
                } else if requires_vehicle {
                    json!({"vehicle_id": EXAMPLE_VEHICLE})
                } else {
                    json!({})
                },
                source_url: source_url.into(),
            },
        )
        .collect();
    for (id, label, kind, description) in [
        (
            "telemetry-subscription",
            "Continuous vehicle stream",
            "subscription",
            "Unavailable: persistent Parallax subscriptions and protobuf decoding are not implemented. Use one-shot readings.",
        ),
        (
            "lock-vehicle",
            "Lock vehicle",
            "mutation",
            "Unavailable: commands require separately reviewed key enrollment, local BLE pairing and signing support.",
        ),
        (
            "unlock-vehicle",
            "Unlock vehicle",
            "mutation",
            "Unavailable: no phone key is enrolled and this app cannot unlock a vehicle.",
        ),
        (
            "set-charge-limit",
            "Set charge limit",
            "mutation",
            "Unavailable: this read-only release does not change vehicle settings.",
        ),
    ] {
        operations.push(Operation {
            id: id.into(),
            label: label.into(),
            family: if kind == "subscription" {
                "Telemetry"
            } else {
                "Commands"
            }
            .into(),
            kind: kind.into(),
            status: "blocked".into(),
            requires_vehicle: true,
            description: description.into(),
            example_variables: if id == "set-charge-limit" {
                json!({"vehicle_id": EXAMPLE_VEHICLE, "limit_percent": 80})
            } else {
                json!({"vehicle_id": EXAMPLE_VEHICLE})
            },
            source_url: "https://rivian-api.kaedenb.org/app/parallax/".into(),
        });
    }
    operations
}

pub fn validate_json_request(operation_id: &str, variables_json: &str) -> ValidationResult {
    if variables_json.len() > MAX_VARIABLE_BYTES {
        return ValidationResult::reject("variables", "Variables exceed the 16 KiB limit");
    }
    match serde_json::from_str(variables_json) {
        Ok(value) => validate_request(operation_id, &value),
        Err(_) => ValidationResult::reject("variables", "Variables must be valid JSON"),
    }
}

pub fn validate_request(operation_id: &str, variables: &Value) -> ValidationResult {
    if let Some(message) = structure_issue(variables) {
        return ValidationResult::reject("variables", message);
    }
    if serde_json::to_vec(variables).map_or(true, |bytes| bytes.len() > MAX_VARIABLE_BYTES) {
        return ValidationResult::reject("variables", "Variables exceed the 16 KiB limit");
    }
    let Some(operation) = catalog().into_iter().find(|entry| entry.id == operation_id) else {
        return ValidationResult::reject("operation_id", "Unknown local operation");
    };
    let Some(object) = variables.as_object() else {
        return ValidationResult::reject("variables", "Variables must be a JSON object");
    };
    let mut issues = Vec::new();
    if operation.status != "live" {
        issues.push(issue(
            "operation_id",
            "Operation is not available in this release",
        ));
    }
    let allowed: &[&str] = match operation_id {
        "charging-history" => &["vehicle_id", "limit"],
        "set-charge-limit" => &["vehicle_id", "limit_percent"],
        _ if operation.requires_vehicle => &["vehicle_id"],
        _ => &[],
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        issues.push(issue(
            "variables",
            "Unknown variable fields are not allowed",
        ));
    }
    if operation.requires_vehicle
        && !object
            .get("vehicle_id")
            .and_then(Value::as_str)
            .is_some_and(valid_id)
    {
        issues.push(issue(
            "vehicle_id",
            "A vehicle ID of 1 to 128 letters, digits, hyphens or underscores is required",
        ));
    }
    if object.get("limit").is_some_and(|value| {
        !value
            .as_u64()
            .is_some_and(|number| (1..=100).contains(&number))
    }) {
        issues.push(issue("limit", "Limit must be an integer from 1 to 100"));
    }
    ValidationResult::from_issues(issues)
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

const USER_QUERY: &str = "query getUserInfo { currentUser { id firstName lastName vehicles { id name vin roles vehicle { model modelYear } } } }";
const STATE_FIELDS: &str = "batteryLevel { value timeStamp } batteryLimit { value timeStamp } distanceToEmpty { value timeStamp } vehicleMileage { value timeStamp } doorFrontLeftLocked { value timeStamp } doorFrontRightLocked { value timeStamp } doorRearLeftLocked { value timeStamp } doorRearRightLocked { value timeStamp } chargerState { value timeStamp } cabinClimateInteriorTemperature { value timeStamp } gnssLocation { latitude longitude timeStamp }";
const DETAIL_FIELDS: &str = "powerState { value timeStamp } gearStatus { value timeStamp } tirePressureStatusFrontLeft { value timeStamp } tirePressureStatusFrontRight { value timeStamp } tirePressureStatusRearLeft { value timeStamp } tirePressureStatusRearRight { value timeStamp } tirePressureStatusValidFrontLeft { value timeStamp } tirePressureStatusValidFrontRight { value timeStamp } tirePressureStatusValidRearLeft { value timeStamp } tirePressureStatusValidRearRight { value timeStamp } doorFrontLeftClosed { value timeStamp } doorFrontRightClosed { value timeStamp } doorRearLeftClosed { value timeStamp } doorRearRightClosed { value timeStamp } closureFrunkClosed { value timeStamp } closureTailgateClosed { value timeStamp } closureLiftgateClosed { value timeStamp } cabinPreconditioningStatus { value timeStamp } petModeStatus { value timeStamp } otaCurrentVersionNumber { value timeStamp } otaCurrentVersionWeek { value timeStamp } otaCurrentVersionYear { value timeStamp } otaStatus { value timeStamp } wiperFluidState { value timeStamp } brakeFluidLow { value timeStamp } cloudConnection { lastSync }";
const HISTORY_QUERY: &str = "query getCompletedSessionSummaries { getCompletedSessionSummaries { transactionId vehicleId vehicleName startInstant endInstant totalEnergyKwh rangeAddedKm chargerType currencyCode paidTotal city vendor isPublic isHomeCharger isRoamingNetwork meta { transactionIdGroupingKey dataSources } } }";

/// Construct only an allowlisted document after revalidation. `user_id` is kept in
/// the stable interface for future reviewed user-scoped queries; current queries
/// derive the account from Rivian's native session, never a browser-supplied ID.
pub fn request(
    operation_id: &str,
    variables: &Value,
    _user_id: Option<&str>,
) -> Result<LiveRequest, CoreError> {
    let operation = catalog()
        .into_iter()
        .find(|entry| entry.id == operation_id)
        .ok_or(CoreError::UnknownOperation)?;
    if operation.status != "live" {
        return Err(CoreError::OperationDisabled);
    }
    if !validate_request(operation_id, variables).valid {
        return Err(CoreError::InvalidRequest);
    }
    let vehicle = variables.get("vehicle_id").cloned().unwrap_or(Value::Null);
    let (endpoint, name, query, upstream_variables) = match operation_id {
        "account-summary" | "list-vehicles" => (LiveEndpoint::Gateway, "getUserInfo", USER_QUERY.into(), json!({})),
        "vehicle-state" | "charging-status" | "vehicle-location" | "vehicle-telemetry" => {
            let detail = if operation_id == "vehicle-telemetry" { DETAIL_FIELDS } else { "" };
            (LiveEndpoint::Gateway, "GetVehicleState", format!("query GetVehicleState($vehicleID: String!) {{ vehicleState(id: $vehicleID) {{ {STATE_FIELDS} {detail} }} }}"), json!({"vehicleID": vehicle}))
        }
        "charging-history" | "account-charging-history" => (LiveEndpoint::Charging, "getCompletedSessionSummaries", HISTORY_QUERY.into(), json!({})),
        "live-charging-session" => (LiveEndpoint::Charging, "getLiveSessionData", "query getLiveSessionData($vehicleId: ID!) { getLiveSessionData(vehicleId: $vehicleId) { chargerId locationId startTime timeElapsed currentCurrency currentPrice isFreeSession isRivianCharger current { value updatedAt } power { value updatedAt } soc { value updatedAt } timeRemaining { value updatedAt } totalChargedEnergy { value updatedAt } rangeAddedThisSession { value updatedAt } vehicleChargerState { value updatedAt } } }".into(), json!({"vehicleId": vehicle})),
        "charging-schedules" => (LiveEndpoint::Gateway, "getVehicleChargingSchedules", "query getVehicleChargingSchedules($vehicleId: String!) { getVehicle(id: $vehicleId) { id chargingSchedules { weekDays startTime duration location { latitude longitude } amperage enabled } } }".into(), json!({"vehicleId": vehicle})),
        "registered-wallboxes" => (LiveEndpoint::Charging, "getRegisteredWallboxes", "query getRegisteredWallboxes { getRegisteredWallboxes { wallboxId userId wifiId name linked latitude longitude chargingStatus power currentVoltage currentAmps softwareVersion model serialNumber maxAmps maxVoltage maxPower } }".into(), json!({})),
        "supported-features" => (LiveEndpoint::Gateway, "SupportedFeatures", "query SupportedFeatures { currentUser { vehicles { id vehicle { vehicleState { supportedFeatures { name status } } } } } }".into(), json!({})),
        "vehicle-connection" => (LiveEndpoint::Gateway, "GetVehicleLastConnection", "query GetVehicleLastConnection($vehicleID: String!) { vehicleState(id: $vehicleID) { cloudConnection { lastSync } } }".into(), json!({"vehicleID": vehicle})),
        "ota-updates" => (LiveEndpoint::Gateway, "getOTAUpdateDetails", "query getOTAUpdateDetails($vehicleId: String!) { getVehicle(id: $vehicleId) { id availableOTAUpdateDetails { url version locale } currentOTAUpdateDetails { url version locale } } }".into(), json!({"vehicleId": vehicle})),
        "vehicle-images" => (LiveEndpoint::Gateway, "getVehicleImages", "query getVehicleImages { getVehicleMobileImages { vehicleId url extension resolution size design placement overlays { url overlay zIndex } } getVehicleOrderMobileImages { orderId vehicleId url extension resolution size design placement overlays { url overlay zIndex } } }".into(), json!({})),
        "drivers-and-keys" => (LiveEndpoint::Gateway, "DriversAndKeys", "query DriversAndKeys($vehicleId: String) { getVehicle(id: $vehicleId) { id vin invitedUsers { ... on ProvisionedUser { firstName lastName email roles userId devices { type mappedIdentityId id hrid deviceName isPaired isEnabled } } ... on UnprovisionedUser { email inviteId status } } } }".into(), json!({"vehicleId": vehicle})),
        "account-profile" => (LiveEndpoint::Orders, "MyRivianDataProfile", "query MyRivianDataProfile { user { userId firstName lastName email { email } phone { formatted } vehicles { id vin highestPriorityRole } } }".into(), json!({})),
        "vehicle-orders" => (LiveEndpoint::Orders, "MyRivianDataOrders", "query MyRivianDataOrders { user { orderSnapshots { id state orderDate type vehicleId vin total paidTotal subtotal fulfillmentSummaryStatus configurationStatus currency } } }".into(), json!({})),
        _ => return Err(CoreError::UnknownOperation),
    };
    Ok(LiveRequest {
        endpoint,
        operation_name: name.into(),
        query,
        variables: upstream_variables,
    })
}

fn response_data(response: &Value) -> Result<&Value, NormalizeError> {
    if response.get("errors").is_some_and(|value| {
        !value.is_null() && value.as_array().is_none_or(|items| !items.is_empty())
    }) {
        return Err(NormalizeError::UpstreamError);
    }
    response
        .get("data")
        .filter(|value| value.is_object())
        .ok_or(NormalizeError::InvalidResponse)
}

pub fn account_identity(response: &Value) -> Result<AccountIdentity, NormalizeError> {
    let data = response_data(response)?;
    let user = data
        .get("currentUser")
        .ok_or(NormalizeError::InvalidResponse)?;
    let user_id = user
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| valid_id(id))
        .ok_or(NormalizeError::InvalidResponse)?
        .to_owned();
    let vehicles = user
        .get("vehicles")
        .and_then(Value::as_array)
        .filter(|items| items.len() <= 1000)
        .ok_or(NormalizeError::InvalidResponse)?;
    let mut seen = HashSet::new();
    let mut vehicle_ids = Vec::new();
    for vehicle in vehicles {
        let id = vehicle
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| valid_id(id))
            .ok_or(NormalizeError::InvalidResponse)?;
        if !seen.insert(id) {
            return Err(NormalizeError::InvalidResponse);
        }
        vehicle_ids.push(id.to_owned());
    }
    Ok(AccountIdentity {
        user_id,
        vehicle_ids,
    })
}

/// Normalize a complete GraphQL envelope. Extra developer operations intentionally
/// return raw `data` under their reviewed document. The native host may additionally
/// include the whole original envelope for its developer response view.
pub fn normalize(
    operation_id: &str,
    variables: &Value,
    response: &Value,
    account_vehicles: &[Value],
) -> Result<Value, NormalizeError> {
    let data = response_data(response)?;
    let operation = catalog()
        .into_iter()
        .find(|entry| entry.id == operation_id && entry.status == "live")
        .ok_or(NormalizeError::UnknownOperation)?;
    if operation.requires_vehicle {
        let id = variables
            .get("vehicle_id")
            .and_then(Value::as_str)
            .ok_or(NormalizeError::VehicleNotOwned)?;
        if !account_vehicles
            .iter()
            .any(|vehicle| vehicle.get("id").and_then(Value::as_str) == Some(id))
        {
            return Err(NormalizeError::VehicleNotOwned);
        }
        for root in ["vehicleState", "getVehicle", "getLiveSessionData"] {
            if let Some(result) = data.get(root) {
                // VehicleState has no ID field in the researched schema. Its
                // association is the authenticated, owned ID in the fixed query.
                // Reject a conflicting echo if a future response adds one.
                for key in ["id", "vehicleId"] {
                    if result
                        .get(key)
                        .is_some_and(|value| value.as_str() != Some(id))
                    {
                        return Err(NormalizeError::VehicleNotOwned);
                    }
                }
            }
        }
        if matches!(
            operation_id,
            "charging-schedules" | "ota-updates" | "drivers-and-keys"
        ) && data["getVehicle"]["id"].as_str() != Some(id)
        {
            return Err(NormalizeError::InvalidResponse);
        }
    }
    match operation_id {
        "account-summary" | "list-vehicles" => {
            let identity = account_identity(response)?;
            let user = &data["currentUser"];
            if operation_id == "account-summary" {
                let display_name = [
                    short_text(&user["firstName"]),
                    short_text(&user["lastName"]),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
                return Ok(
                    json!({"id": identity.user_id, "display_name": if display_name.is_empty() { "Rivian account" } else { &display_name }, "vehicle_count": identity.vehicle_ids.len()}),
                );
            }
            let vehicles: Vec<Value> = user["vehicles"]
                .as_array()
                .ok_or(NormalizeError::InvalidResponse)?
                .iter()
                .map(normalize_account_vehicle)
                .collect();
            Ok(json!({"vehicles": vehicles}))
        }
        "vehicle-state" | "charging-status" | "vehicle-location" => {
            let id = variables
                .get("vehicle_id")
                .and_then(Value::as_str)
                .ok_or(NormalizeError::VehicleNotOwned)?;
            let vehicle = account_vehicles
                .iter()
                .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
                .ok_or(NormalizeError::VehicleNotOwned)?;
            let state = data
                .get("vehicleState")
                .filter(|value| value.is_object())
                .ok_or(NormalizeError::InvalidResponse)?;
            let location = normalize_location(&state["gnssLocation"]);
            if operation_id == "vehicle-location" {
                return Ok(
                    json!({"vehicle_id": id, "location": location, "observed_at": timestamp(&state["gnssLocation"]["timeStamp"])}),
                );
            }
            if operation_id == "charging-status" {
                return Ok(
                    json!({"vehicle_id": id, "state": charging_state(&state["chargerState"]["value"]), "battery_percent": number(&state["batteryLevel"]["value"], 0.0, 100.0), "limit_percent": number(&state["batteryLimit"]["value"], 0.0, 100.0), "power_kw": null, "field_observed_at": field_times(state)}),
                );
            }
            let mut result = vehicle.clone();
            let result_object = result
                .as_object_mut()
                .ok_or(NormalizeError::InvalidResponse)?;
            for (key, value) in [
                (
                    "battery_percent",
                    json!(number(&state["batteryLevel"]["value"], 0.0, 100.0)),
                ),
                (
                    "estimated_range_km",
                    json!(number(&state["distanceToEmpty"]["value"], 0.0, 5000.0)),
                ),
                (
                    "odometer_km",
                    json!(
                        number(&state["vehicleMileage"]["value"], 0.0, 10_000_000_000.0)
                            .map(|meters| meters / 1000.0)
                    ),
                ),
                (
                    "temperature_celsius",
                    json!(number(
                        &state["cabinClimateInteriorTemperature"]["value"],
                        -100.0,
                        100.0
                    )),
                ),
                ("locked", json!(door_locked(state))),
                (
                    "charging_state",
                    json!(charging_state(&state["chargerState"]["value"])),
                ),
                ("location", location),
                ("observed_at", Value::Null),
                ("field_observed_at", field_times(state)),
                ("source", json!(SOURCE)),
            ] {
                result_object.insert(key.into(), value);
            }
            Ok(json!({"vehicle": result}))
        }
        "charging-history" => normalize_history(variables, data, account_vehicles),
        _ if catalog()
            .iter()
            .any(|entry| entry.id == operation_id && entry.status == "live") =>
        {
            Ok(data.clone())
        }
        _ => Err(NormalizeError::UnknownOperation),
    }
}

fn normalize_account_vehicle(vehicle: &Value) -> Value {
    let model = short_text(&vehicle["vehicle"]["model"]).unwrap_or("Rivian");
    let name = short_text(&vehicle["name"]).unwrap_or(model);
    let year = vehicle["vehicle"]["modelYear"]
        .as_u64()
        .or_else(|| {
            vehicle["vehicle"]["modelYear"]
                .as_str()
                .and_then(|year| year.parse().ok())
        })
        .filter(|year| (2000..=2200).contains(year));
    json!({"id": vehicle["id"], "name": name, "model": model, "model_year": year,
        "battery_percent": null, "estimated_range_km": null, "odometer_km": null,
        "locked": null, "charging_state": "unknown", "temperature_celsius": null,
        "location": null, "observed_at": null, "source": SOURCE})
}

fn normalize_location(value: &Value) -> Value {
    match (
        number(&value["latitude"], -90.0, 90.0),
        number(&value["longitude"], -180.0, 180.0),
    ) {
        (Some(latitude), Some(longitude)) => {
            json!({"latitude": latitude, "longitude": longitude, "accuracy_m": null})
        }
        _ => Value::Null,
    }
}

fn door_locked(state: &Value) -> Option<bool> {
    let values: Vec<Option<&str>> = [
        "doorFrontLeftLocked",
        "doorFrontRightLocked",
        "doorRearLeftLocked",
        "doorRearRightLocked",
    ]
    .iter()
    .map(|key| state[*key]["value"].as_str())
    .collect();
    if values.contains(&Some("unlocked")) {
        Some(false)
    } else if values.iter().all(|value| *value == Some("locked")) {
        Some(true)
    } else {
        None
    }
}

fn charging_state(value: &Value) -> &'static str {
    match value.as_str() {
        Some("charging_active" | "charging") => "charging",
        Some("charging_complete" | "complete") => "complete",
        Some("charging_ready" | "charging_scheduled" | "charging_paused") => "plugged_in",
        Some("charger_disconnected" | "disconnected" | "charging_disconnected") => "disconnected",
        _ => "unknown",
    }
}

fn number(value: &Value, minimum: f64, maximum: f64) -> Option<f64> {
    value
        .as_f64()
        .filter(|number| number.is_finite() && (minimum..=maximum).contains(number))
}

fn short_text(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|text| !text.is_empty() && text.len() <= 256 && !text.chars().any(char::is_control))
}

fn timestamp(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|text| text.len() <= 40 && OffsetDateTime::parse(text, &Rfc3339).is_ok())
}

fn field_times(state: &Value) -> Value {
    let mut times = serde_json::Map::new();
    if let Some(state) = state.as_object() {
        for (key, reading) in state {
            if let Some(time) = timestamp(&reading["timeStamp"]) {
                times.insert(key.clone(), json!(time));
            }
        }
    }
    Value::Object(times)
}

fn normalize_history(
    variables: &Value,
    data: &Value,
    account_vehicles: &[Value],
) -> Result<Value, NormalizeError> {
    let id = variables
        .get("vehicle_id")
        .and_then(Value::as_str)
        .ok_or(NormalizeError::VehicleNotOwned)?;
    if !account_vehicles
        .iter()
        .any(|vehicle| vehicle.get("id").and_then(Value::as_str) == Some(id))
    {
        return Err(NormalizeError::VehicleNotOwned);
    }
    let entries = data["getCompletedSessionSummaries"]
        .as_array()
        .filter(|items| items.len() <= 20_000)
        .ok_or(NormalizeError::InvalidResponse)?;
    let limit = variables["limit"].as_u64().unwrap_or(100).min(100) as usize;
    let unassigned_count = entries
        .iter()
        .filter(|entry| entry["vehicleId"].is_null())
        .count();
    let mut seen = HashSet::new();
    let mut sessions: Vec<Value> = entries.iter().filter(|entry| entry["vehicleId"].as_str() == Some(id)).filter_map(|entry| {
        let session_id = short_text(&entry["transactionId"])?;
        if !seen.insert(session_id) { return None; }
        let start = timestamp(&entry["startInstant"]);
        let end = timestamp(&entry["endInstant"]);
        let duration = start.and_then(|start| OffsetDateTime::parse(start, &Rfc3339).ok()).zip(end.and_then(|end| OffsetDateTime::parse(end, &Rfc3339).ok())).map(|(start, end)| (end - start).as_seconds_f64() / 60.0).filter(|minutes| (0.0..=100_000.0).contains(minutes));
        Some(json!({"id": session_id, "started_at": start, "ended_at": end, "energy_kwh": number(&entry["totalEnergyKwh"], 0.0, 1000.0), "duration_minutes": duration}))
    }).collect();
    sessions.sort_by(|a, b| {
        let key = |value: &Value| {
            value["started_at"]
                .as_str()
                .and_then(|text| OffsetDateTime::parse(text, &Rfc3339).ok())
        };
        key(b).cmp(&key(a))
    });
    let has_more = sessions.len() > limit;
    sessions.truncate(limit);
    Ok(
        json!({"vehicle_id": id, "sessions": sessions, "has_more": has_more, "unassigned_count": unassigned_count}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> Value {
        json!({"data":{"currentUser":{"id":"owner-1","firstName":"Test","lastName":"Owner","vehicles":[{"id":"vehicle-1","name":"My R1T","vehicle":{"model":"R1T","modelYear":"2025"}}]}}})
    }

    fn garage() -> Vec<Value> {
        normalize("list-vehicles", &json!({}), &account(), &[]).unwrap()["vehicles"]
            .as_array()
            .unwrap()
            .clone()
    }

    #[test]
    fn every_live_catalog_example_has_exactly_one_fixed_query() {
        for operation in catalog() {
            let result = request(&operation.id, &operation.example_variables, None);
            assert_eq!(
                result.is_ok(),
                operation.status == "live",
                "{}",
                operation.id
            );
            if let Ok(request) = result {
                assert!(request.query.starts_with("query "));
                assert!(
                    request
                        .endpoint
                        .url()
                        .starts_with("https://rivian.com/api/gql/")
                );
            }
        }
    }

    #[test]
    fn live_fixture_expectations_and_resource_bounds_hold() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/live-validation-cases.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            assert_eq!(
                validate_json_request(
                    case["operation_id"].as_str().unwrap(),
                    case["variables_json"].as_str().unwrap()
                )
                .valid,
                case["expected_valid"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
        }
        assert!(
            !validate_json_request("account-summary", &" ".repeat(MAX_VARIABLE_BYTES + 1)).valid
        );
        let mut deep = json!({});
        for _ in 0..9 {
            deep = json!({"nested": deep});
        }
        assert!(!validate_request("account-summary", &deep).valid);
        assert!(!validate_request("account-summary", &json!({"items": vec![0; 257]})).valid);
    }

    #[test]
    fn request_rejects_url_documents_tokens_and_mutations() {
        for variables in [
            json!({"vehicle_id":"vehicle-1","query":"mutation Delete"}),
            json!({"vehicle_id":"https://example.com"}),
            json!({"vehicle_id":"vehicle-1","u-sess":"secret"}),
            json!({"vehicle_id":"vehicle-1","url":"https://example.com"}),
        ] {
            assert!(request("vehicle-state", &variables, None).is_err());
        }
        assert!(request("unlock-vehicle", &json!({"vehicle_id":"vehicle-1"}), None).is_err());
        assert!(request("anything", &json!({}), None).is_err());
        let request = request("vehicle-state", &json!({"vehicle_id":"vehicle-1"}), None).unwrap();
        assert_eq!(request.variables, json!({"vehicleID":"vehicle-1"}));
        assert!(!request.query.contains("vehicle-1"));
    }

    #[test]
    fn account_identity_requires_valid_unique_authoritative_ids() {
        assert_eq!(
            account_identity(&account()).unwrap().vehicle_ids,
            ["vehicle-1"]
        );
        for response in [
            json!({"data":{"currentUser":{"id":"owner-1"}}}),
            json!({"data":{"currentUser":{"id":"owner-1","vehicles":[{"id":"vehicle-1"},{"id":"vehicle-1"}]}}}),
            json!({"data":{"currentUser":{"id":"owner-1","vehicles":[{"id":"../vehicle"}]}}}),
        ] {
            assert!(account_identity(&response).is_err());
        }
    }

    #[test]
    fn missing_readings_are_null_and_never_newly_timestamped() {
        let vehicles = garage();
        assert_eq!(vehicles[0]["model_year"], 2025);
        let result = normalize(
            "vehicle-state",
            &json!({"vehicle_id":"vehicle-1"}),
            &json!({"data":{"vehicleState":{}}}),
            &vehicles,
        )
        .unwrap();
        for key in [
            "battery_percent",
            "odometer_km",
            "locked",
            "location",
            "observed_at",
        ] {
            assert!(result["vehicle"][key].is_null(), "{key}");
        }
        assert_eq!(result["vehicle"]["source"], "live");
        assert_eq!(result["vehicle"]["charging_state"], "unknown");
    }

    #[test]
    fn normalization_converts_meters_preserves_field_times_and_requires_all_door_locks() {
        let state = json!({"data":{"vehicleState":{"vehicleMileage":{"value":12345678,"timeStamp":"2026-10-01T01:02:03Z"},"distanceToEmpty":{"value":321},"batteryLevel":{"value":74.2},"doorFrontLeftLocked":{"value":"locked"},"chargerState":{"value":"charging_ready"},"gnssLocation":{"latitude":0,"longitude":0,"timeStamp":"2026-09-30T01:02:03Z"}}}});
        let result = normalize(
            "vehicle-state",
            &json!({"vehicle_id":"vehicle-1"}),
            &state,
            &garage(),
        )
        .unwrap();
        assert_eq!(result["vehicle"]["odometer_km"], 12345.678);
        assert_eq!(result["vehicle"]["estimated_range_km"], 321.0);
        assert_eq!(
            result["vehicle"]["field_observed_at"]["vehicleMileage"],
            "2026-10-01T01:02:03Z"
        );
        assert!(result["vehicle"]["locked"].is_null());
        assert_eq!(result["vehicle"]["location"]["latitude"], 0.0);
        assert!(result["vehicle"]["location"]["accuracy_m"].is_null());
        assert_eq!(result["vehicle"]["charging_state"], "plugged_in");
    }

    #[test]
    fn invalid_readings_and_cross_vehicle_requests_fail_closed() {
        let response = json!({"data":{"vehicleState":{"batteryLevel":{"value":101},"distanceToEmpty":{"value":-1},"gnssLocation":{"latitude":91,"longitude":0}}}});
        let result = normalize(
            "vehicle-state",
            &json!({"vehicle_id":"vehicle-1"}),
            &response,
            &garage(),
        )
        .unwrap();
        assert!(result["vehicle"]["battery_percent"].is_null());
        assert!(result["vehicle"]["estimated_range_km"].is_null());
        assert!(result["vehicle"]["location"].is_null());
        assert_eq!(
            normalize(
                "vehicle-state",
                &json!({"vehicle_id":"vehicle-2"}),
                &response,
                &garage()
            ),
            Err(NormalizeError::VehicleNotOwned)
        );
        assert_eq!(
            normalize(
                "vehicle-state",
                &json!({"vehicle_id":"vehicle-1"}),
                &json!({"data":{"vehicleState":{}},"errors":[{"message":"sensitive upstream text"}]}),
                &garage()
            ),
            Err(NormalizeError::UpstreamError)
        );
        assert_eq!(
            normalize(
                "vehicle-state",
                &json!({"vehicle_id":"vehicle-1"}),
                &json!({"data":{"vehicleState":{"id":"vehicle-2"}}}),
                &garage()
            ),
            Err(NormalizeError::VehicleNotOwned)
        );
        assert_eq!(
            normalize(
                "charging-schedules",
                &json!({"vehicle_id":"vehicle-1"}),
                &json!({"data":{"getVehicle":{"chargingSchedules":[]}}}),
                &garage()
            ),
            Err(NormalizeError::InvalidResponse)
        );
    }

    #[test]
    fn history_filters_unassigned_and_other_vehicles_sorts_bounds_and_computes_duration() {
        let session = |id: &str, vehicle: Value, start: &str| json!({"transactionId":id,"vehicleId":vehicle,"startInstant":start,"endInstant":"2026-10-02T02:00:00Z","totalEnergyKwh":20});
        let response = json!({"data":{"getCompletedSessionSummaries":[session("earlier",json!("vehicle-1"),"2026-10-01T00:00:00Z"),session("unassigned",Value::Null,"2026-10-02T00:00:00Z"),session("other",json!("vehicle-2"),"2026-10-02T00:00:00Z"),session("latest",json!("vehicle-1"),"2026-10-02T00:00:00Z"),session("latest",json!("vehicle-1"),"2026-10-02T00:00:00Z")]}});
        let result = normalize(
            "charging-history",
            &json!({"vehicle_id":"vehicle-1","limit":1}),
            &response,
            &garage(),
        )
        .unwrap();
        assert_eq!(result["sessions"].as_array().unwrap().len(), 1);
        assert_eq!(result["sessions"][0]["id"], "latest");
        assert_eq!(result["sessions"][0]["duration_minutes"], 120.0);
        assert_eq!(result["has_more"], true);
        assert_eq!(result["unassigned_count"], 1);
        assert_eq!(normalize("account-charging-history", &json!({}), &response, &garage()).unwrap()["getCompletedSessionSummaries"].as_array().unwrap().len(), 5);
    }
}
