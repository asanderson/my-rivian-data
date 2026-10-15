//! Bounded, non-secret Parallax decoding and timestamp-aware partial state.
//!
//! Wire tags/units were reviewed against the reverse-engineered community schema
//! at rivian-python-client commit 7d6028233752cc226faeae35e9d12ce387abb2fe
//! (2026-10-14), `src/rivian/parallax/proto/{body,comfort,dynamics,energy,charging}.proto`.
//! This is an original decoder: no client implementation or generated code is copied.
//! https://github.com/bretterer/rivian-python-client/tree/7d6028233752cc226faeae35e9d12ce387abb2fe/src/rivian/parallax/proto
//!
//! RivDocs supplies the RVM/transport catalog. Its battery range and charging-time
//! descriptions differ from the pinned schema, so range uses its dedicated topic.
//! Unlike proto3 generated accessors, missing scalar wire tags do not become zero.
//! An explicitly encoded zero is accepted. Empty payloads preserve previous ages.
//! Authority, credentials, connection lifecycle and vehicle ownership stay native.

use std::collections::{BTreeMap, VecDeque};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub const READ_RVMS: &[&str] = &[
    "energy.high_voltage.battery_state",
    "charging.session.status",
    "charging.session.soc_slider",
    "charging.session.power",
    "charging.session.time_estimation",
    "dynamics.vehicle.range",
    "dynamics.vehicle.odometer",
    "dynamics.vehicle.gnss",
    "body.locks.states",
    "comfort.cabin.cabin_temperatures",
    "dynamics.tires.state",
];
pub const MAX_PAYLOAD_BYTES: usize = 32 * 1024;
pub const MAX_BASE64_BYTES: usize = MAX_PAYLOAD_BYTES.div_ceil(3) * 4;
pub const MAX_DEVELOPER_ENTRIES: usize = 16;
pub const MAX_DEVELOPER_PAYLOAD_BYTES: usize = 1024;
const MAX_TOPIC_BYTES: usize = 128;
const MAX_TRACKED_TOPICS: usize = 64;
const MAX_MESSAGE_FIELDS: usize = 128;
pub const SCHEMA_REVISION: &str = "7d6028233752cc226faeae35e9d12ce387abb2fe";
const POSITIONS: [&str; 4] = ["front_left", "front_right", "rear_left", "rear_right"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateOutcome {
    Applied,
    Empty,
    Unknown,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParallaxError {
    #[error("Invalid vehicle identifier")]
    InvalidVehicle,
    #[error("Invalid telemetry topic")]
    InvalidTopic,
    #[error("Invalid telemetry timestamp")]
    InvalidTimestamp,
    #[error("Telemetry payload is too large")]
    PayloadTooLarge,
    #[error("Malformed telemetry payload")]
    MalformedPayload,
    #[error("Invalid telemetry reading")]
    InvalidReading,
    #[error("Out-of-order telemetry message")]
    StaleMessage,
    #[error("Telemetry resource limit exceeded")]
    ResourceLimit,
}

#[derive(Clone, Debug)]
struct Reading {
    value: Value,
    timestamp_ms: i64,
}

#[derive(Debug)]
pub struct TelemetryState {
    vehicle_id: String,
    readings: BTreeMap<&'static str, Reading>,
    doors: [Option<Reading>; 4],
    tires: [BTreeMap<&'static str, Reading>; 4],
    topic_times: BTreeMap<String, i64>,
    developer: VecDeque<Value>,
}

#[derive(Default)]
struct Patch {
    readings: Vec<(&'static str, Value, i64)>,
    doors: Vec<(usize, Value, i64)>,
    tires: Vec<(usize, &'static str, Value, i64)>,
}

impl TelemetryState {
    pub fn new(vehicle_id: impl Into<String>) -> Result<Self, ParallaxError> {
        let vehicle_id = vehicle_id.into();
        if vehicle_id.is_empty()
            || vehicle_id.len() > 128
            || !vehicle_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(ParallaxError::InvalidVehicle);
        }
        Ok(Self {
            vehicle_id,
            readings: BTreeMap::new(),
            doors: std::array::from_fn(|_| None),
            tires: std::array::from_fn(|_| BTreeMap::new()),
            // Reserve the fixed owner topics: arbitrary unknown topics cannot
            // exhaust the map and prevent valid subscribed readings.
            topic_times: READ_RVMS.iter().map(|rvm| ((*rvm).into(), -1)).collect(),
            developer: VecDeque::new(),
        })
    }

    /// Validate and decode completely before mutating any reading or timestamp.
    pub fn apply(
        &mut self,
        rvm: &str,
        payload_base64: &str,
        timestamp_ms: i64,
    ) -> Result<UpdateOutcome, ParallaxError> {
        if rvm.is_empty()
            || rvm.len() > MAX_TOPIC_BYTES
            || !rvm
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(ParallaxError::InvalidTopic);
        }
        timestamp_text(timestamp_ms)?;
        if payload_base64.len() > MAX_BASE64_BYTES {
            return Err(ParallaxError::PayloadTooLarge);
        }
        if self
            .topic_times
            .get(rvm)
            .is_some_and(|last| timestamp_ms <= *last)
        {
            return Err(ParallaxError::StaleMessage);
        }
        if !self.topic_times.contains_key(rvm) && self.topic_times.len() >= MAX_TRACKED_TOPICS {
            return Err(ParallaxError::ResourceLimit);
        }
        let bytes = STANDARD
            .decode(payload_base64)
            .map_err(|_| ParallaxError::MalformedPayload)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(ParallaxError::PayloadTooLarge);
        }
        let message = Message::parse(&bytes)?;
        let known = READ_RVMS.contains(&rvm);
        let patch = if known {
            decode(rvm, &message, timestamp_ms)?
        } else {
            Patch::default()
        };
        self.topic_times.insert(rvm.into(), timestamp_ms);
        for (key, value, timestamp_ms) in patch.readings {
            merge(&mut self.readings, key, value, timestamp_ms);
        }
        for (position, value, timestamp_ms) in patch.doors {
            if self.doors[position]
                .as_ref()
                .is_none_or(|old| timestamp_ms > old.timestamp_ms)
            {
                self.doors[position] = Some(Reading {
                    value,
                    timestamp_ms,
                });
            }
        }
        for (position, key, value, timestamp_ms) in patch.tires {
            merge(&mut self.tires[position], key, value, timestamp_ms);
        }
        if !known {
            if self.developer.len() == MAX_DEVELOPER_ENTRIES {
                self.developer.pop_front();
            }
            let shown = payload_base64.len().min(MAX_DEVELOPER_PAYLOAD_BYTES);
            self.developer.push_back(json!({"rvm":rvm,"timestamp_ms":timestamp_ms,
                "observed_at":timestamp_text(timestamp_ms)?,"payload_base64":&payload_base64[..shown],
                "payload_bytes":bytes.len(),"truncated":shown<payload_base64.len(),"decoded":false}));
            return Ok(UpdateOutcome::Unknown);
        }
        Ok(if bytes.is_empty() {
            UpdateOutcome::Empty
        } else {
            UpdateOutcome::Applied
        })
    }

    /// Flat owner reading shape, with no raw protobuf or session information.
    pub fn snapshot(&self) -> Value {
        let mut snapshot = json!({"id":self.vehicle_id,"source":"live","observed_at":null,
            "battery_percent":null,"pack_energy_kwh":null,"estimated_range_km":null,
            "odometer_km":null,"locked":null,"charging_state":"unknown",
            "temperature_celsius":null,"location":null,"limit_percent":null,
            "power_kw":null,"time_remaining_minutes":null,"door_locks":{},"tires":{},"field_observed_at":{}});
        let mut times = serde_json::Map::new();
        for (&key, reading) in &self.readings {
            if key.starts_with('_') {
                continue;
            }
            snapshot[key] = reading.value.clone();
            times.insert(key.into(), json!(timestamp_text(reading.timestamp_ms).ok()));
        }
        let (charging_state, charging_time) = self.charging_state();
        snapshot["charging_state"] = json!(charging_state);
        if let Some(timestamp) = charging_time {
            times.insert(
                "charging_state".into(),
                json!(timestamp_text(timestamp).ok()),
            );
        }
        // Upstream does not necessarily resend zero power when charging ends.
        if charging_state != "charging" {
            snapshot["power_kw"] = Value::Null;
            snapshot["time_remaining_minutes"] = Value::Null;
        }
        let unlocked = self
            .doors
            .iter()
            .flatten()
            .filter(|reading| reading.value == false)
            .max_by_key(|reading| reading.timestamp_ms);
        let locked_time = if let Some(reading) = unlocked {
            snapshot["locked"] = json!(false);
            Some(reading.timestamp_ms)
        } else if self
            .doors
            .iter()
            .all(|door| door.as_ref().is_some_and(|reading| reading.value == true))
        {
            snapshot["locked"] = json!(true);
            self.doors
                .iter()
                .flatten()
                .map(|reading| reading.timestamp_ms)
                .min()
        } else {
            None
        };
        if let Some(timestamp) = locked_time {
            times.insert("locked".into(), json!(timestamp_text(timestamp).ok()));
        }
        for (position, name) in POSITIONS.iter().enumerate() {
            if let Some(reading) = &self.doors[position] {
                snapshot["door_locks"][*name] = reading.value.clone();
                times.insert(
                    format!("door_locks.{name}"),
                    json!(timestamp_text(reading.timestamp_ms).ok()),
                );
            } else {
                snapshot["door_locks"][*name] = Value::Null;
            }
            let mut tire =
                json!({"pressure_bar":null,"status":null,"validity":"unknown","observed_at":null});
            for (&key, reading) in &self.tires[position] {
                tire[key] = reading.value.clone();
                times.insert(
                    format!("tires.{name}.{key}"),
                    json!(timestamp_text(reading.timestamp_ms).ok()),
                );
            }
            // The age is the oldest pressure/status included, not a fresh
            // validity envelope that happened to omit the measurement.
            let age = ["pressure_bar", "status"]
                .into_iter()
                .filter_map(|key| self.tires[position].get(key))
                .map(|reading| reading.timestamp_ms)
                .min()
                .or_else(|| {
                    self.tires[position]
                        .get("validity")
                        .map(|reading| reading.timestamp_ms)
                });
            if let Some(age) = age {
                tire["observed_at"] = json!(timestamp_text(age).ok());
            }
            snapshot["tires"][*name] = tire;
        }
        snapshot["field_observed_at"] = Value::Object(times);
        snapshot
    }

    pub fn developer_entries(&self) -> Vec<Value> {
        self.developer.iter().cloned().collect()
    }

    fn charging_state(&self) -> (&'static str, Option<i64>) {
        let connection = self.readings.get("_connection_state");
        let display = self.readings.get("_display_state");
        if let Some(connection) = connection {
            if connection.value == 1 {
                // A newer active display with an older disconnected plug is a
                // contradictory partial report, not evidence of either state.
                if display.is_some_and(|reading| {
                    matches!(reading.value.as_u64(), Some(3 | 25))
                        && reading.timestamp_ms > connection.timestamp_ms
                }) {
                    return ("unknown", display.map(|reading| reading.timestamp_ms));
                }
                return ("disconnected", Some(connection.timestamp_ms));
            }
            if connection.value != 2 {
                return ("unknown", Some(connection.timestamp_ms));
            }
        }
        let state = match display.and_then(|reading| reading.value.as_u64()) {
            Some(3 | 25) => "charging",
            Some(4) => "complete",
            Some(1 | 2 | 5 | 8 | 9 | 17 | 19 | 24)
                if connection.is_some_and(|reading| reading.value == 2) =>
            {
                "plugged_in"
            }
            _ => "unknown",
        };
        let age = if state == "plugged_in" {
            connection
                .zip(display)
                .map(|(a, b)| a.timestamp_ms.min(b.timestamp_ms))
        } else {
            display.map(|reading| reading.timestamp_ms)
        };
        (state, age)
    }
}

fn merge(
    map: &mut BTreeMap<&'static str, Reading>,
    key: &'static str,
    value: Value,
    timestamp_ms: i64,
) {
    if map
        .get(key)
        .is_none_or(|old| timestamp_ms > old.timestamp_ms)
    {
        map.insert(
            key,
            Reading {
                value,
                timestamp_ms,
            },
        );
    }
}

fn timestamp_text(timestamp_ms: i64) -> Result<String, ParallaxError> {
    if timestamp_ms < 0 {
        return Err(ParallaxError::InvalidTimestamp);
    }
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(timestamp_ms) * 1_000_000)
        .map_err(|_| ParallaxError::InvalidTimestamp)?
        .format(&Rfc3339)
        .map_err(|_| ParallaxError::InvalidTimestamp)
}

fn bounded(value: f64, minimum: f64, maximum: f64) -> Result<f64, ParallaxError> {
    if value.is_finite() && (minimum..=maximum).contains(&value) {
        Ok(value)
    } else {
        Err(ParallaxError::InvalidReading)
    }
}

fn decode(rvm: &str, message: &Message<'_>, timestamp: i64) -> Result<Patch, ParallaxError> {
    let mut patch = Patch::default();
    match rvm {
        "energy.high_voltage.battery_state" => {
            if let Some(charge) = message.child(1)? {
                if let Some(soc) = charge.double(1)? {
                    patch.readings.push((
                        "battery_percent",
                        json!(bounded(soc, 0.0, 100.0)?),
                        timestamp,
                    ));
                }
                if let Some(energy) = charge.double(2)? {
                    patch.readings.push((
                        "pack_energy_kwh",
                        json!(bounded(energy, 0.0, 1000.0)?),
                        timestamp,
                    ));
                }
            }
            if let Some(temperatures) = message.child(2)? {
                for tag in 1..=3 {
                    temperatures.float(tag)?;
                }
            }
            if let Some(event) = message.child(3)? {
                for tag in 1..=2 {
                    if event.integer(tag)?.is_some_and(|value| value > 1) {
                        return Err(ParallaxError::InvalidReading);
                    }
                }
            }
            message.integer(4)?;
            if message.integer(5)?.is_some_and(|value| value > 1) {
                return Err(ParallaxError::InvalidReading);
            }
            message.integer(6)?;
        }
        "dynamics.vehicle.range" => {
            if let Some(value) = message.integer(1)? {
                patch.readings.push((
                    "estimated_range_km",
                    json!(bounded(value as f64, 0.0, 5000.0)?),
                    timestamp,
                ));
            }
            message.integer(2)?;
            message.integer(3)?;
        }
        "dynamics.vehicle.odometer" => {
            if let Some(value) = message.integer(1)? {
                patch.readings.push((
                    "odometer_km",
                    json!(bounded(value as f64, 0.0, 10_000_000.0)?),
                    timestamp,
                ));
            }
        }
        "dynamics.vehicle.gnss" => {
            let latitude = message
                .double(1)?
                .map(|value| bounded(value, -90.0, 90.0))
                .transpose()?;
            let longitude = message
                .double(2)?
                .map(|value| bounded(value, -180.0, 180.0))
                .transpose()?;
            message.double(3)?;
            for tag in 4..=9 {
                message.float(tag)?;
            }
            let accuracy = message
                .float(6)?
                .map(|value| bounded(value, 0.0, 100_000.0))
                .transpose()?;
            let fix = message
                .integer(10)?
                .map(|value| {
                    let mut fix =
                        i64::try_from(value).map_err(|_| ParallaxError::InvalidTimestamp)?;
                    // The pinned client documents GPS-epoch ms on some vehicles.
                    if fix > 0 && fix < 1_577_836_800_000 {
                        fix = fix
                            .checked_add(315_964_800_000 - 18_000)
                            .ok_or(ParallaxError::InvalidTimestamp)?;
                    }
                    reading_time(fix, timestamp)
                })
                .transpose()?
                .unwrap_or(timestamp);
            if let (Some(latitude), Some(longitude)) = (latitude, longitude) {
                patch.readings.push((
                    "location",
                    json!({"latitude":latitude,"longitude":longitude,"accuracy_m":accuracy}),
                    fix,
                ));
            }
        }
        "body.locks.states" => {
            let mut seen = [false; 4];
            for entry in message.children(1)? {
                let id = entry.integer(1)?;
                let state = entry.integer(2)?;
                entry.integer(3)?;
                if let Some(position) = id
                    .filter(|id| (1..=4).contains(id))
                    .map(|id| id as usize - 1)
                {
                    if seen[position] {
                        return Err(ParallaxError::MalformedPayload);
                    }
                    seen[position] = true;
                    if let Some(state) = state {
                        patch.doors.push((
                            position,
                            match state {
                                1 => json!(true),
                                2 | 3 => json!(false),
                                _ => Value::Null,
                            },
                            timestamp,
                        ));
                    }
                }
            }
        }
        "comfort.cabin.cabin_temperatures" => {
            for zone in message.children(1)? {
                zone.integer(1)?;
                zone.float(2)?;
            }
            message.float(2)?;
            message.float(4)?;
            if let Some(value) = message.float(3)? {
                patch.readings.push((
                    "temperature_celsius",
                    json!(bounded(value, -100.0, 100.0)?),
                    timestamp,
                ));
            }
        }
        "dynamics.tires.state" => {
            message.integer(1)?;
            let mut seen = [false; 4];
            for tire in message.children(2)? {
                let position = tire.integer(1)?;
                let status = tire.integer(2)?;
                let pressure = tire.double(3)?;
                let validity = tire.integer(4)?.unwrap_or(0);
                let measured = tire
                    .integer(5)?
                    .map(|value| {
                        i64::try_from(value)
                            .map_err(|_| ParallaxError::InvalidTimestamp)
                            .and_then(|time| reading_time(time, timestamp))
                    })
                    .transpose()?
                    .unwrap_or(timestamp);
                if let Some(position) = position
                    .filter(|id| (1..=4).contains(id))
                    .map(|id| id as usize - 1)
                {
                    if seen[position] {
                        return Err(ParallaxError::MalformedPayload);
                    }
                    seen[position] = true;
                    patch.tires.push((
                        position,
                        "validity",
                        json!(match validity {
                            0 => "valid",
                            1 => "invalid",
                            _ => "unknown",
                        }),
                        measured,
                    ));
                    if validity == 0 {
                        if let Some(pressure) = pressure {
                            patch.tires.push((
                                position,
                                "pressure_bar",
                                json!(bounded(pressure, 0.0, 10.0)?),
                                measured,
                            ));
                        }
                        if let Some(status) = status {
                            patch.tires.push((
                                position,
                                "status",
                                match status {
                                    1 => json!("normal"),
                                    2 => json!("warning_hard"),
                                    3 => json!("warning_soft"),
                                    4 => json!("warning_puncture"),
                                    _ => Value::Null,
                                },
                                measured,
                            ));
                        }
                    } else {
                        patch
                            .tires
                            .push((position, "pressure_bar", Value::Null, measured));
                        patch
                            .tires
                            .push((position, "status", Value::Null, measured));
                    }
                }
            }
        }
        "charging.session.status" => {
            if let Some(value) = message.integer(1)? {
                patch
                    .readings
                    .push(("_connection_state", json!(value), timestamp));
            }
            if let Some(value) = message.integer(2)? {
                patch
                    .readings
                    .push(("_display_state", json!(value), timestamp));
            }
            message.integer(3)?;
        }
        "charging.session.soc_slider" => {
            if let Some(value) = message.integer(1)? {
                patch.readings.push((
                    "limit_percent",
                    json!(bounded(value as f64, 0.0, 100.0)?),
                    timestamp,
                ));
            }
        }
        "charging.session.power" => {
            if let Some(value) = message.float(1)? {
                patch
                    .readings
                    .push(("power_kw", json!(bounded(value, 0.0, 1000.0)?), timestamp));
            }
        }
        "charging.session.time_estimation" => {
            let validity = message.integer(1)?;
            let remaining = message.integer(2)?;
            if validity == Some(1) {
                if let Some(remaining) = remaining {
                    patch.readings.push((
                        "time_remaining_minutes",
                        json!(bounded(remaining as f64, 0.0, 100_000.0)?),
                        timestamp,
                    ));
                }
            } else if validity.is_some() {
                patch
                    .readings
                    .push(("time_remaining_minutes", Value::Null, timestamp));
            }
        }
        _ => {}
    }
    Ok(patch)
}

fn reading_time(time: i64, envelope: i64) -> Result<i64, ParallaxError> {
    timestamp_text(time)?;
    if time > envelope {
        return Err(ParallaxError::InvalidTimestamp);
    }
    Ok(time)
}

#[derive(Clone, Copy)]
enum Wire<'a> {
    Integer(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32(u32),
}

struct Message<'a> {
    fields: Vec<(u32, Wire<'a>)>,
}

impl<'a> Message<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, ParallaxError> {
        let mut cursor = 0;
        let mut fields = Vec::new();
        while cursor < bytes.len() {
            if fields.len() >= MAX_MESSAGE_FIELDS {
                return Err(ParallaxError::ResourceLimit);
            }
            let key = read_varint(bytes, &mut cursor)?;
            let tag = key >> 3;
            if tag == 0 || tag > 0x1fff_ffff {
                return Err(ParallaxError::MalformedPayload);
            }
            let wire = match key & 7 {
                0 => Wire::Integer(read_varint(bytes, &mut cursor)?),
                1 => Wire::Fixed64(u64::from_le_bytes(
                    take(bytes, &mut cursor, 8)?
                        .try_into()
                        .map_err(|_| ParallaxError::MalformedPayload)?,
                )),
                2 => {
                    let length = usize::try_from(read_varint(bytes, &mut cursor)?)
                        .map_err(|_| ParallaxError::MalformedPayload)?;
                    Wire::Bytes(take(bytes, &mut cursor, length)?)
                }
                5 => Wire::Fixed32(u32::from_le_bytes(
                    take(bytes, &mut cursor, 4)?
                        .try_into()
                        .map_err(|_| ParallaxError::MalformedPayload)?,
                )),
                _ => return Err(ParallaxError::MalformedPayload),
            };
            fields.push((tag as u32, wire));
        }
        Ok(Self { fields })
    }
    fn single(&self, tag: u32) -> Result<Option<Wire<'a>>, ParallaxError> {
        let mut matching = self
            .fields
            .iter()
            .filter(|(field, _)| *field == tag)
            .map(|(_, wire)| *wire);
        let first = matching.next();
        if matching.next().is_some() {
            return Err(ParallaxError::MalformedPayload);
        }
        Ok(first)
    }
    fn integer(&self, tag: u32) -> Result<Option<u64>, ParallaxError> {
        match self.single(tag)? {
            None => Ok(None),
            Some(Wire::Integer(value)) => Ok(Some(value)),
            _ => Err(ParallaxError::MalformedPayload),
        }
    }
    fn double(&self, tag: u32) -> Result<Option<f64>, ParallaxError> {
        match self.single(tag)? {
            None => Ok(None),
            Some(Wire::Fixed64(value)) => {
                let value = f64::from_bits(value);
                if value.is_finite() {
                    Ok(Some(value))
                } else {
                    Err(ParallaxError::InvalidReading)
                }
            }
            _ => Err(ParallaxError::MalformedPayload),
        }
    }
    fn float(&self, tag: u32) -> Result<Option<f64>, ParallaxError> {
        match self.single(tag)? {
            None => Ok(None),
            Some(Wire::Fixed32(value)) => {
                let value = f32::from_bits(value);
                if value.is_finite() {
                    Ok(Some(f64::from(value)))
                } else {
                    Err(ParallaxError::InvalidReading)
                }
            }
            _ => Err(ParallaxError::MalformedPayload),
        }
    }
    fn child(&self, tag: u32) -> Result<Option<Message<'a>>, ParallaxError> {
        match self.single(tag)? {
            None => Ok(None),
            Some(Wire::Bytes(bytes)) => Ok(Some(Self::parse(bytes)?)),
            _ => Err(ParallaxError::MalformedPayload),
        }
    }
    fn children(&self, tag: u32) -> Result<Vec<Message<'a>>, ParallaxError> {
        self.fields
            .iter()
            .filter(|(field, _)| *field == tag)
            .map(|(_, wire)| match wire {
                Wire::Bytes(bytes) => Self::parse(bytes),
                _ => Err(ParallaxError::MalformedPayload),
            })
            .collect()
    }
}

fn read_varint(bytes: &[u8], cursor: &mut usize) -> Result<u64, ParallaxError> {
    let mut value = 0;
    for shift in (0..=63).step_by(7) {
        let byte = *bytes.get(*cursor).ok_or(ParallaxError::MalformedPayload)?;
        *cursor += 1;
        if shift == 63 && byte > 1 {
            return Err(ParallaxError::MalformedPayload);
        }
        value |= u64::from(byte & 127) << shift;
        if byte & 128 == 0 {
            return Ok(value);
        }
    }
    Err(ParallaxError::MalformedPayload)
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, length: usize) -> Result<&'a [u8], ParallaxError> {
    let end = cursor
        .checked_add(length)
        .ok_or(ParallaxError::MalformedPayload)?;
    let result = bytes
        .get(*cursor..end)
        .ok_or(ParallaxError::MalformedPayload)?;
    *cursor = end;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
    const NOW: i64 = 1_791_979_200_000;
    fn state() -> TelemetryState {
        TelemetryState::new("vehicle-1").unwrap()
    }
    fn encoded(bytes: &[u8]) -> String {
        STANDARD.encode(bytes)
    }
    fn varint(value: u64) -> Vec<u8> {
        let mut output = Vec::new();
        let mut value = value;
        loop {
            let byte = (value & 127) as u8;
            value >>= 7;
            output.push(byte | if value != 0 { 128 } else { 0 });
            if value == 0 {
                break;
            }
        }
        output
    }
    fn integer(tag: u8, value: u64) -> Vec<u8> {
        let mut out = vec![tag << 3];
        out.extend(varint(value));
        out
    }
    fn double(tag: u8, value: f64) -> Vec<u8> {
        let mut out = vec![(tag << 3) | 1];
        out.extend(value.to_le_bytes());
        out
    }
    fn float(tag: u8, value: f32) -> Vec<u8> {
        let mut out = vec![(tag << 3) | 5];
        out.extend(value.to_le_bytes());
        out
    }
    fn child(tag: u8, bytes: &[u8]) -> Vec<u8> {
        let mut out = vec![(tag << 3) | 2];
        out.extend(varint(bytes.len() as u64));
        out.extend(bytes);
        out
    }
    fn apply(state: &mut TelemetryState, rvm: &str, bytes: &[u8], timestamp: i64) {
        state.apply(rvm, &encoded(bytes), timestamp).unwrap();
    }

    #[test]
    fn documented_wire_fixture_decodes_owner_readings_with_original_ages() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/parallax-readings.json")).unwrap();
        let mut state = state();
        for reading in fixture["messages"].as_array().unwrap() {
            assert_eq!(
                state
                    .apply(
                        reading["rvm"].as_str().unwrap(),
                        reading["payload"].as_str().unwrap(),
                        reading["timestamp"].as_i64().unwrap()
                    )
                    .unwrap(),
                UpdateOutcome::Applied
            );
        }
        let snapshot = state.snapshot();
        for (key, value) in fixture["expected"].as_object().unwrap() {
            assert_eq!(&snapshot[key], value, "{key}");
        }
        assert_eq!(
            snapshot["field_observed_at"]["battery_percent"],
            "2026-10-14T12:00:00Z"
        );
        assert_eq!(
            snapshot["field_observed_at"]["estimated_range_km"],
            "2026-10-14T12:00:01Z"
        );
        assert!(snapshot.get("payload").is_none());
    }

    #[test]
    fn absent_fields_stay_unavailable_and_explicit_zero_is_preserved() {
        let mut state = state();
        for key in [
            "battery_percent",
            "estimated_range_km",
            "odometer_km",
            "locked",
            "temperature_celsius",
            "location",
            "power_kw",
        ] {
            assert!(state.snapshot()[key].is_null());
        }
        apply(
            &mut state,
            "energy.high_voltage.battery_state",
            &child(1, &double(1, 65.0)),
            NOW,
        );
        apply(
            &mut state,
            "energy.high_voltage.battery_state",
            &child(1, &double(2, 50.0)),
            NOW + 1000,
        );
        assert_eq!(state.snapshot()["battery_percent"], 65.0);
        assert_eq!(
            state.snapshot()["field_observed_at"]["battery_percent"],
            "2026-10-14T12:00:00Z"
        );
        apply(
            &mut state,
            "energy.high_voltage.battery_state",
            &child(1, &double(1, 0.0)),
            NOW + 2000,
        );
        assert_eq!(state.snapshot()["battery_percent"], 0.0);
        apply(&mut state, "dynamics.vehicle.gnss", &double(1, 47.2), NOW);
        assert!(state.snapshot()["location"].is_null());
        apply(
            &mut state,
            "dynamics.vehicle.gnss",
            &[double(1, 0.0), double(2, 0.0)].concat(),
            NOW + 1000,
        );
        assert_eq!(state.snapshot()["location"]["latitude"], 0.0);
        assert_eq!(state.snapshot()["location"]["longitude"], 0.0);
        assert!(state.snapshot()["location"]["accuracy_m"].is_null());
    }

    #[test]
    fn malformed_stale_and_oversize_messages_leave_state_unchanged() {
        let mut state = state();
        apply(
            &mut state,
            "energy.high_voltage.battery_state",
            &child(1, &double(1, 65.0)),
            NOW,
        );
        let before = state.snapshot();
        assert_eq!(
            state.apply("energy.high_voltage.battery_state", "CgkJAAAAAABAUEA=", NOW),
            Err(ParallaxError::StaleMessage)
        );
        for bytes in [
            vec![0],
            vec![10, 9, 9],
            vec![8, 255, 255, 255, 255, 255, 255, 255, 255, 255, 2],
            vec![11],
            child(1, &integer(1, 65)),
            child(1, &[double(1, 70.0), double(1, 80.0)].concat()),
        ] {
            assert!(
                state
                    .apply(
                        "energy.high_voltage.battery_state",
                        &encoded(&bytes),
                        NOW + 1
                    )
                    .is_err(),
                "{bytes:?}"
            );
        }
        assert_eq!(
            state.apply("energy.high_voltage.battery_state", "%%%", NOW + 1),
            Err(ParallaxError::MalformedPayload)
        );
        assert_eq!(
            state.apply(
                "energy.high_voltage.battery_state",
                &"A".repeat(MAX_BASE64_BYTES + 1),
                NOW + 1
            ),
            Err(ParallaxError::PayloadTooLarge)
        );
        assert_eq!(
            state.apply(
                "energy.high_voltage.battery_state",
                &encoded(&vec![0; MAX_PAYLOAD_BYTES + 1]),
                NOW + 1
            ),
            Err(ParallaxError::PayloadTooLarge)
        );
        assert_eq!(state.snapshot(), before);
    }

    #[test]
    fn partial_locks_do_not_invent_locked_or_false_and_keep_each_age() {
        let mut state = state();
        apply(
            &mut state,
            "body.locks.states",
            &child(1, &[integer(1, 1), integer(2, 1)].concat()),
            NOW,
        );
        assert!(state.snapshot()["locked"].is_null());
        for id in 2..=4 {
            apply(
                &mut state,
                "body.locks.states",
                &child(1, &[integer(1, id), integer(2, 1)].concat()),
                NOW + id as i64 * 1000,
            );
        }
        assert_eq!(state.snapshot()["locked"], true);
        assert_eq!(
            state.snapshot()["field_observed_at"]["locked"],
            "2026-10-14T12:00:00Z"
        );
        apply(
            &mut state,
            "body.locks.states",
            &child(1, &[integer(1, 1), integer(3, 1)].concat()),
            NOW + 5000,
        );
        assert_eq!(state.snapshot()["locked"], true);
        assert_eq!(
            state.snapshot()["field_observed_at"]["locked"],
            "2026-10-14T12:00:00Z"
        );
        apply(
            &mut state,
            "body.locks.states",
            &child(1, &[integer(1, 1), integer(2, 2)].concat()),
            NOW + 6000,
        );
        assert_eq!(state.snapshot()["locked"], false);
        apply(
            &mut state,
            "body.locks.states",
            &child(1, &[integer(1, 1), integer(2, 99)].concat()),
            NOW + 7000,
        );
        assert!(state.snapshot()["locked"].is_null());
    }

    #[test]
    fn tire_measurement_ages_and_invalidity_do_not_republish_stale_values() {
        let mut state = state();
        let tire = |pressure, validity, timestamp| {
            child(
                2,
                &[
                    integer(1, 1),
                    integer(2, 1),
                    double(3, pressure),
                    integer(4, validity),
                    integer(5, timestamp),
                ]
                .concat(),
            )
        };
        apply(
            &mut state,
            "dynamics.tires.state",
            &tire(3.0, 0, NOW as u64),
            NOW,
        );
        apply(
            &mut state,
            "dynamics.tires.state",
            &tire(2.0, 0, (NOW - 1000) as u64),
            NOW + 1000,
        );
        assert_eq!(state.snapshot()["tires"]["front_left"]["pressure_bar"], 3.0);
        assert_eq!(
            state.snapshot()["tires"]["front_left"]["observed_at"],
            "2026-10-14T12:00:00Z"
        );
        apply(
            &mut state,
            "dynamics.tires.state",
            &tire(4.0, 1, (NOW + 2000) as u64),
            NOW + 2000,
        );
        assert!(state.snapshot()["tires"]["front_left"]["pressure_bar"].is_null());
        assert!(state.snapshot()["tires"]["front_left"]["status"].is_null());
        assert_eq!(
            state.snapshot()["tires"]["front_left"]["validity"],
            "invalid"
        );
    }

    #[test]
    fn charging_power_is_hidden_after_completion_and_missing_fields_keep_age() {
        let mut state = state();
        apply(
            &mut state,
            "charging.session.status",
            &[integer(1, 2), integer(2, 3)].concat(),
            NOW,
        );
        apply(&mut state, "charging.session.power", &float(1, 11.5), NOW);
        assert_eq!(state.snapshot()["charging_state"], "charging");
        assert_eq!(state.snapshot()["power_kw"], 11.5);
        apply(
            &mut state,
            "charging.session.status",
            &integer(2, 4),
            NOW + 1000,
        );
        assert_eq!(state.snapshot()["charging_state"], "complete");
        assert!(state.snapshot()["power_kw"].is_null());
        apply(
            &mut state,
            "charging.session.status",
            &integer(1, 1),
            NOW + 2000,
        );
        assert_eq!(state.snapshot()["charging_state"], "disconnected");
        assert_eq!(
            state.apply("charging.session.status", "", NOW + 3000),
            Ok(UpdateOutcome::Empty)
        );
        assert_eq!(
            state.snapshot()["field_observed_at"]["charging_state"],
            "2026-10-14T12:00:02Z"
        );
    }

    #[test]
    fn unknown_topics_are_separate_bounded_and_do_not_create_owner_readings() {
        let mut state = state();
        let before = state.snapshot();
        for id in 0..32 {
            assert_eq!(
                state.apply(
                    &format!("unknown.topic_{id}"),
                    &encoded(&integer(1, 1)),
                    NOW + id
                ),
                Ok(UpdateOutcome::Unknown)
            );
        }
        assert_eq!(state.snapshot(), before);
        assert_eq!(state.developer_entries().len(), MAX_DEVELOPER_ENTRIES);
        assert!(
            state
                .developer_entries()
                .iter()
                .all(|entry| entry["payload_base64"].as_str().unwrap().len() <= 1024)
        );
    }

    #[test]
    fn identifiers_timestamps_and_invalid_numbers_fail_closed() {
        for id in ["", "../vehicle", "https://example.com"] {
            assert!(TelemetryState::new(id).is_err());
        }
        let mut state = state();
        assert_eq!(
            state.apply("bad topic", "", NOW),
            Err(ParallaxError::InvalidTopic)
        );
        assert_eq!(
            state.apply("dynamics.vehicle.range", "", -1),
            Err(ParallaxError::InvalidTimestamp)
        );
        for value in [f64::NAN, f64::INFINITY, -1.0, 101.0] {
            assert_eq!(
                state.apply(
                    "energy.high_voltage.battery_state",
                    &encoded(&child(1, &double(1, value))),
                    NOW
                ),
                Err(ParallaxError::InvalidReading)
            );
        }
        assert!(state.snapshot()["battery_percent"].is_null());
    }

    #[test]
    fn contradictory_charging_ages_and_unknown_connections_stay_unknown() {
        let mut state = state();
        apply(
            &mut state,
            "charging.session.status",
            &[integer(1, 1), integer(2, 1)].concat(),
            NOW,
        );
        apply(
            &mut state,
            "charging.session.status",
            &integer(2, 3),
            NOW + 1000,
        );
        assert_eq!(state.snapshot()["charging_state"], "unknown");
        apply(
            &mut state,
            "charging.session.status",
            &integer(1, 2),
            NOW + 2000,
        );
        assert_eq!(state.snapshot()["charging_state"], "charging");
        assert_eq!(
            state.snapshot()["field_observed_at"]["charging_state"],
            "2026-10-14T12:00:01Z"
        );
        apply(
            &mut state,
            "charging.session.status",
            &integer(1, 99),
            NOW + 3000,
        );
        assert_eq!(state.snapshot()["charging_state"], "unknown");
    }

    #[test]
    fn new_envelopes_do_not_replace_newer_gnss_fix_and_source_units_are_explicit() {
        let mut state = state();
        let fix =
            |latitude, time| [double(1, latitude), double(2, -122.3), integer(10, time)].concat();
        apply(
            &mut state,
            "dynamics.vehicle.gnss",
            &fix(47.2, NOW as u64),
            NOW,
        );
        apply(
            &mut state,
            "dynamics.vehicle.gnss",
            &fix(48.0, (NOW - 1000) as u64),
            NOW + 1000,
        );
        assert_eq!(state.snapshot()["location"]["latitude"], 47.2);
        let gps_ms = NOW - 315_964_800_000 + 18_000;
        apply(
            &mut state,
            "dynamics.vehicle.gnss",
            &fix(49.0, gps_ms as u64),
            NOW + 2000,
        );
        // Same original fix time is a duplicate, even in a newer envelope.
        assert_eq!(state.snapshot()["location"]["latitude"], 47.2);
        let before = state.snapshot();
        assert_eq!(
            state.apply(
                "dynamics.vehicle.gnss",
                &encoded(&fix(50.0, (NOW + 5000) as u64)),
                NOW + 3000
            ),
            Err(ParallaxError::InvalidTimestamp)
        );
        assert_eq!(state.snapshot(), before);
    }

    #[test]
    fn resource_bounds_do_not_block_known_topics_and_published_bad_sample_is_rejected() {
        let mut state = state();
        for id in 0..(MAX_TRACKED_TOPICS - READ_RVMS.len()) {
            state
                .apply(
                    &format!("unknown.topic_{id}"),
                    &encoded(&integer(1, 1)),
                    NOW,
                )
                .unwrap();
        }
        assert_eq!(
            state.apply("unknown.extra", &encoded(&integer(1, 1)), NOW),
            Err(ParallaxError::ResourceLimit)
        );
        apply(
            &mut state,
            "energy.high_voltage.battery_state",
            &child(1, &double(1, 65.0)),
            NOW,
        );
        assert_eq!(state.snapshot()["battery_percent"], 65.0);
        assert_eq!(
            state.apply(
                "energy.high_voltage.battery_state",
                "CkIKBAAAADMzT0ARAAAAQOESYEASDw00MytCFWdmOkIdzcwUQhoAIAEwCQ==",
                NOW + 1
            ),
            Err(ParallaxError::MalformedPayload)
        );
        assert_eq!(
            state.apply(
                "unknown.fields",
                &encoded(&integer(1, 1).repeat(MAX_MESSAGE_FIELDS + 1)),
                NOW
            ),
            Err(ParallaxError::ResourceLimit)
        );
        let mut state = TelemetryState::new("vehicle-1").unwrap();
        assert_eq!(
            state.apply(
                "unknown.fields",
                &encoded(&integer(1, 1).repeat(MAX_MESSAGE_FIELDS + 1)),
                NOW
            ),
            Err(ParallaxError::ResourceLimit)
        );
        let unknown = child(15, &vec![0; 2048]);
        state
            .apply("unknown.blob", &encoded(&unknown), NOW)
            .unwrap();
        assert_eq!(
            state.developer_entries()[0]["payload_base64"]
                .as_str()
                .unwrap()
                .len(),
            MAX_DEVELOPER_PAYLOAD_BYTES
        );
        assert_eq!(state.developer_entries()[0]["truncated"], true);
    }
}
