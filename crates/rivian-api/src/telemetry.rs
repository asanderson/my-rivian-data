//! Lifecycle-bound native Parallax reads. Live account compatibility is unverified.
//!
//! Reviewed 2026-10-14: https://rivian-api.kaedenb.org/app/parallax/subscription/
//! and https://github.com/kaedenbrinkman/rivian-api/blob/2eb98b4b50b26d9f80a190f65107d4bd9d828cd9/examples/parallax-subscribe.py
//! Protocol: https://github.com/enisdenjo/graphql-ws/blob/master/PROTOCOL.md
//! These references describe an unofficial contract, not live-account proof.
//! Both request and response admission use the eleven portable `READ_RVMS`
//! reviewed against schema revision 7d6028233752cc226faeae35e9d12ce387abb2fe.
//! Unrequested payloads, including native key material, are never retained.

use crate::{ApiError, SubscriptionCredentials};
use futures_util::{SinkExt, StreamExt};
use rivian_core::parallax::{READ_RVMS, TelemetryState};
use serde::Serialize;
use serde_json::{Value, json};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, watch};
use tokio::task::JoinHandle;
use tokio::time::{Instant, timeout};
use tokio_tungstenite::tungstenite::{
    Error as WsError, Message, client::IntoClientRequest, http::HeaderValue,
    protocol::WebSocketConfig,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async_with_config};

const ENDPOINT: &str = "wss://api.rivian.com/gql-consumer-subscriptions/graphql";
const PROTOCOL: &str = "graphql-transport-ws";
const MAX_FRAME_BYTES: usize = 131_072;
const MAX_RECONNECT_ATTEMPTS: usize = 8;
type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Stopped,
    Connecting,
    Connected,
    Reconnecting,
    AuthenticationRequired,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct TelemetrySnapshot {
    /// Native failure provenance; browser serialization never grants authority.
    #[serde(skip)]
    pub authorization_epoch: Option<u64>,
    pub state: ConnectionState,
    pub vehicle_id: Option<String>,
    pub compatibility: &'static str,
    pub readings: Value,
    pub raw_topics: Vec<Value>,
    pub error_code: Option<&'static str>,
}

struct State {
    generation: u64,
    authorization_epoch: Option<u64>,
    connection: ConnectionState,
    telemetry: Option<TelemetryState>,
    revocation: Option<(u64, watch::Receiver<u64>)>,
    error_code: Option<&'static str>,
}

impl State {
    fn empty() -> Self {
        Self {
            generation: 0,
            authorization_epoch: None,
            connection: ConnectionState::Stopped,
            telemetry: None,
            revocation: None,
            error_code: None,
        }
    }
    fn clear(&mut self, error: Option<&'static str>) {
        self.telemetry = None;
        self.revocation = None;
        self.error_code = error;
        // Preserve the exact origin of an upstream authentication failure after
        // clearing readings. A later account generation must never be blamed
        // for a previous worker's failure or a local revocation diagnostic.
        let authentication_failure = error == Some("account_auth_required");
        if !authentication_failure {
            self.authorization_epoch = None;
        }
        self.connection = if authentication_failure {
            ConnectionState::AuthenticationRequired
        } else {
            ConnectionState::Stopped
        };
    }
}

struct Worker {
    cancel: watch::Sender<bool>,
    task: JoinHandle<()>,
}

#[derive(Clone, Copy)]
struct Timings {
    handshake: Duration,
    acknowledgement: Duration,
    write: Duration,
    heartbeat: Duration,
    idle: Duration,
    retry_initial: Duration,
    retry_maximum: Duration,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            handshake: Duration::from_secs(10),
            acknowledgement: Duration::from_secs(10),
            write: Duration::from_secs(5),
            heartbeat: Duration::from_secs(30),
            idle: Duration::from_secs(60),
            retry_initial: Duration::from_secs(1),
            retry_maximum: Duration::from_secs(30),
        }
    }
}

/// One native selected vehicle, one bounded worker, and no caller-controlled URL.
/// Credentials are opaque capabilities created only after native ownership checks.
pub struct TelemetryService {
    state: Arc<Mutex<State>>,
    worker: Mutex<Option<Worker>>,
    gate: Mutex<()>,
    timings: Timings,
    // No production environment variable, request or CLI can set this endpoint.
    #[cfg(test)]
    test_endpoint: Option<String>,
}

impl Default for TelemetryService {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State::empty())),
            worker: Mutex::new(None),
            gate: Mutex::new(()),
            timings: Timings::default(),
            #[cfg(test)]
            test_endpoint: None,
        }
    }
    pub async fn start(&self, credentials: SubscriptionCredentials) -> Result<(), ApiError> {
        if !credentials.is_current() {
            return Err(ApiError::new(
                "session_revoked",
                "This local session has ended. Reopen My Rivian Data.",
            ));
        }
        let telemetry = TelemetryState::new(credentials.vehicle_id.clone())
            .map_err(|_| ApiError::new("invalid_operation", "Choose a valid account vehicle."))?;
        // Reject header injection before superseding an existing worker.
        let _ = request(ENDPOINT, &credentials).map_err(|_| ApiError::schema())?;
        let _gate = self.gate.lock().await;
        // A start queued behind a newer generation must not cancel its worker.
        if !credentials.is_current() {
            return Err(ApiError::new(
                "session_revoked",
                "This account connection has changed.",
            ));
        }
        self.stop_worker().await;
        if !credentials.is_current() {
            return Err(ApiError::new(
                "session_revoked",
                "This local session has ended. Reopen My Rivian Data.",
            ));
        }
        let generation = {
            let mut state = self.state.lock().await;
            state.generation = state.generation.wrapping_add(1);
            state.connection = ConnectionState::Connecting;
            state.authorization_epoch = Some(credentials.epoch);
            state.telemetry = Some(telemetry);
            state.revocation = Some((credentials.epoch, credentials.revocation.clone()));
            state.error_code = None;
            state.generation
        };
        let (cancel, canceled) = watch::channel(false);
        let state = Arc::clone(&self.state);
        let timings = self.timings;
        let endpoint = ENDPOINT.to_owned();
        #[cfg(test)]
        let endpoint = self.test_endpoint.clone().unwrap_or(endpoint);
        let task = tokio::spawn(run(
            credentials,
            state,
            generation,
            canceled,
            endpoint,
            timings,
        ));
        *self.worker.lock().await = Some(Worker { cancel, task });
        Ok(())
    }
    pub async fn snapshot(&self) -> TelemetrySnapshot {
        let mut state = self.state.lock().await;
        if state.revocation.as_ref().is_some_and(|(epoch, receiver)| {
            *receiver.borrow() != *epoch || receiver.has_changed().is_err()
        }) {
            state.clear(Some("session_revoked"));
        }
        let readings = state
            .telemetry
            .as_ref()
            .map(TelemetryState::snapshot)
            .unwrap_or(Value::Null);
        TelemetrySnapshot {
            authorization_epoch: state.authorization_epoch,
            state: state.connection,
            vehicle_id: readings
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned),
            compatibility: "unverified",
            raw_topics: state
                .telemetry
                .as_ref()
                .map(TelemetryState::developer_entries)
                .unwrap_or_default(),
            readings,
            error_code: state.error_code,
        }
    }
    pub async fn stop(&self) {
        let _gate = self.gate.lock().await;
        {
            let mut state = self.state.lock().await;
            state.generation = state.generation.wrapping_add(1);
            state.clear(None);
        }
        self.stop_worker().await;
    }
    pub async fn stop_at_epoch(&self, expected_epoch: u64) -> Result<(), ApiError> {
        let _gate = self.gate.lock().await;
        {
            let mut state = self.state.lock().await;
            if state
                .authorization_epoch
                .is_some_and(|epoch| epoch != expected_epoch)
            {
                return Err(ApiError::new(
                    "session_revoked",
                    "This account connection has changed.",
                ));
            }
            state.generation = state.generation.wrapping_add(1);
            state.clear(None);
        }
        self.stop_worker().await;
        Ok(())
    }
    async fn stop_worker(&self) {
        if let Some(worker) = self.worker.lock().await.take() {
            let _ = worker.cancel.send(true);
            // Every worker IO/sleep observes cancellation; this timeout also
            // prevents a damaged worker from retaining native credentials.
            let mut task = worker.task;
            if timeout(Duration::from_millis(500), &mut task)
                .await
                .is_err()
            {
                task.abort();
                let _ = task.await;
            }
        }
    }
    #[cfg(test)]
    fn test_new(endpoint: String) -> Self {
        let mut service = Self::new();
        service.test_endpoint = Some(endpoint);
        service.timings = Timings {
            handshake: Duration::from_millis(250),
            acknowledgement: Duration::from_millis(100),
            write: Duration::from_millis(100),
            heartbeat: Duration::from_secs(1),
            idle: Duration::from_secs(2),
            retry_initial: Duration::from_millis(10),
            retry_maximum: Duration::from_millis(50),
        };
        service
    }
}

impl Drop for TelemetryService {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.get_mut().take() {
            let _ = worker.cancel.send(true);
            worker.task.abort();
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Failure {
    Canceled,
    Revoked,
    Retry(&'static str),
    Fatal(&'static str),
    Authentication,
}

fn request(
    endpoint: &str,
    credentials: &SubscriptionCredentials,
) -> Result<tokio_tungstenite::tungstenite::handshake::client::Request, Failure> {
    let mut request = endpoint
        .into_client_request()
        .map_err(|_| Failure::Fatal("telemetry_protocol"))?;
    request
        .headers_mut()
        .insert("sec-websocket-protocol", HeaderValue::from_static(PROTOCOL));
    request.headers_mut().insert(
        "apollographql-client-name",
        HeaderValue::from_static("com.rivian.android.consumer"),
    );
    for (name, secret) in [
        ("a-sess", &credentials.app_session),
        ("u-sess", &credentials.user_session),
        ("csrf-token", &credentials.csrf),
    ] {
        let mut value =
            HeaderValue::from_str(secret).map_err(|_| Failure::Fatal("telemetry_protocol"))?;
        value.set_sensitive(true);
        request.headers_mut().insert(name, value);
    }
    Ok(request)
}

fn websocket_error(error: WsError) -> Failure {
    match error {
        WsError::Http(response) if matches!(response.status().as_u16(), 401 | 403) => {
            Failure::Authentication
        }
        WsError::Http(response) if response.status().is_redirection() => {
            Failure::Fatal("telemetry_redirect_refused")
        }
        WsError::Capacity(_) => Failure::Fatal("telemetry_frame_too_large"),
        WsError::Protocol(_) | WsError::Utf8(_) => Failure::Fatal("telemetry_protocol"),
        _ => Failure::Retry("upstream_unavailable"),
    }
}

async fn guarded<T>(
    future: impl Future<Output = Result<T, Failure>>,
    cancel: &mut watch::Receiver<bool>,
    revocation: &mut watch::Receiver<u64>,
    epoch: u64,
) -> Result<T, Failure> {
    if *cancel.borrow() {
        return Err(Failure::Canceled);
    }
    if *revocation.borrow() != epoch || revocation.has_changed().is_err() {
        return Err(Failure::Revoked);
    }
    tokio::select! {
        biased;
        _ = cancel.changed() => Err(Failure::Canceled),
        _ = revocation.changed() => Err(Failure::Revoked),
        result = future => {
            if *revocation.borrow() != epoch { Err(Failure::Revoked) } else { result }
        }
    }
}

async fn status(
    state: &Mutex<State>,
    generation: u64,
    connection: ConnectionState,
    error: Option<&'static str>,
) {
    let mut state = state.lock().await;
    if state.generation == generation {
        state.connection = connection;
        state.error_code = error;
    }
}

async fn run(
    credentials: SubscriptionCredentials,
    state: Arc<Mutex<State>>,
    generation: u64,
    mut cancel: watch::Receiver<bool>,
    endpoint: String,
    timings: Timings,
) {
    let mut revocation = credentials.revocation.clone();
    let mut attempts = 0;
    let mut backoff = timings.retry_initial;
    let result = loop {
        let started = Instant::now();
        let result = connection(
            &credentials,
            &state,
            generation,
            &mut cancel,
            &mut revocation,
            &endpoint,
            timings,
        )
        .await;
        let Err(Failure::Retry(code)) = result else {
            break result;
        };
        // A stable connection gives the next outage a new retry budget. Rapid
        // failures cannot turn into unlimited socket/credential attempts.
        if started.elapsed() >= timings.idle {
            attempts = 0;
            backoff = timings.retry_initial;
        }
        attempts += 1;
        if attempts > MAX_RECONNECT_ATTEMPTS {
            break Err(Failure::Fatal("telemetry_reconnect_exhausted"));
        }
        status(
            &state,
            generation,
            ConnectionState::Reconnecting,
            Some(code),
        )
        .await;
        let waited = guarded(
            async {
                tokio::time::sleep(backoff).await;
                Ok(())
            },
            &mut cancel,
            &mut revocation,
            credentials.epoch,
        )
        .await;
        if let Err(error) = waited {
            break Err(error);
        }
        backoff = backoff.saturating_mul(2).min(timings.retry_maximum);
    };
    match result {
        Err(Failure::Canceled) => {}
        Err(Failure::Revoked) => {
            let mut state = state.lock().await;
            if state.generation == generation {
                state.clear(Some("session_revoked"));
            }
        }
        Err(Failure::Authentication) => {
            let mut state = state.lock().await;
            if state.generation == generation {
                state.clear(Some("account_auth_required"));
            }
        }
        Err(Failure::Fatal(code)) => {
            status(&state, generation, ConnectionState::Error, Some(code)).await
        }
        Err(Failure::Retry(code)) => {
            status(
                &state,
                generation,
                ConnectionState::Reconnecting,
                Some(code),
            )
            .await
        }
        Ok(()) => status(&state, generation, ConnectionState::Stopped, None).await,
    }
}

async fn send(socket: &mut Socket, value: Value, duration: Duration) -> Result<(), Failure> {
    timeout(
        duration,
        socket.send(Message::Text(value.to_string().into())),
    )
    .await
    .map_err(|_| Failure::Retry("telemetry_write_timeout"))?
    .map_err(websocket_error)
}

async fn receive(socket: &mut Socket) -> Result<Value, Failure> {
    loop {
        match socket.next().await {
            Some(Ok(Message::Text(text))) => {
                if text.len() > MAX_FRAME_BYTES {
                    return Err(Failure::Fatal("telemetry_frame_too_large"));
                }
                return serde_json::from_str(&text)
                    .map_err(|_| Failure::Fatal("telemetry_protocol"));
            }
            Some(Ok(Message::Ping(payload))) => socket
                .send(Message::Pong(payload))
                .await
                .map_err(websocket_error)?,
            Some(Ok(Message::Pong(_))) => {}
            Some(Ok(Message::Close(frame))) => {
                return Err(
                    if frame.is_some_and(|frame| matches!(u16::from(frame.code), 4401 | 4403)) {
                        Failure::Authentication
                    } else {
                        Failure::Retry("telemetry_disconnected")
                    },
                );
            }
            Some(Ok(_)) => return Err(Failure::Fatal("telemetry_protocol")),
            Some(Err(error)) => return Err(websocket_error(error)),
            None => return Err(Failure::Retry("telemetry_disconnected")),
        }
    }
}

async fn connection(
    credentials: &SubscriptionCredentials,
    state: &Mutex<State>,
    generation: u64,
    cancel: &mut watch::Receiver<bool>,
    revocation: &mut watch::Receiver<u64>,
    endpoint: &str,
    timings: Timings,
) -> Result<(), Failure> {
    let request = request(endpoint, credentials)?;
    let config = WebSocketConfig::default()
        .read_buffer_size(4096)
        .write_buffer_size(0)
        .max_write_buffer_size(MAX_FRAME_BYTES)
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));
    // connect_async makes one verified rustls handshake. Non-101 HTTP responses
    // are errors; it never follows a redirect or reads proxy environment values.
    let (mut socket, response) = guarded(
        async {
            timeout(
                timings.handshake,
                connect_async_with_config(request, Some(config), false),
            )
            .await
            .map_err(|_| Failure::Retry("telemetry_handshake_timeout"))?
            .map_err(websocket_error)
        },
        cancel,
        revocation,
        credentials.epoch,
    )
    .await?;
    if response
        .headers()
        .get("sec-websocket-protocol")
        .and_then(|value| value.to_str().ok())
        != Some(PROTOCOL)
    {
        return Err(Failure::Fatal("telemetry_protocol"));
    }
    let result = connected(
        &mut socket,
        credentials,
        state,
        generation,
        cancel,
        revocation,
        timings,
    )
    .await;
    if matches!(result, Err(Failure::Canceled) | Err(Failure::Revoked)) {
        // Bound best-effort completion. Dropping the socket also closes it.
        let _ = send(
            &mut socket,
            json!({"id":SUBSCRIPTION_ID,"type":"complete"}),
            Duration::from_millis(100),
        )
        .await;
    }
    result
}

async fn connected(
    socket: &mut Socket,
    credentials: &SubscriptionCredentials,
    state: &Mutex<State>,
    generation: u64,
    cancel: &mut watch::Receiver<bool>,
    revocation: &mut watch::Receiver<u64>,
    timings: Timings,
) -> Result<(), Failure> {
    guarded(
        send(socket, json!({"type":"connection_init"}), timings.write),
        cancel,
        revocation,
        credentials.epoch,
    )
    .await?;
    let acknowledgement_deadline = Instant::now() + timings.acknowledgement;
    loop {
        let frame = guarded(
            async {
                tokio::time::timeout_at(acknowledgement_deadline, receive(socket))
                    .await
                    .map_err(|_| Failure::Retry("telemetry_ack_timeout"))?
            },
            cancel,
            revocation,
            credentials.epoch,
        )
        .await?;
        match frame.get("type").and_then(Value::as_str) {
            Some("connection_ack") => break,
            Some("ping") => {
                guarded(
                    send(socket, pong(&frame), timings.write),
                    cancel,
                    revocation,
                    credentials.epoch,
                )
                .await?;
            }
            Some("pong") => {}
            Some("connection_error") => return Err(Failure::Authentication),
            _ => return Err(Failure::Fatal("telemetry_protocol")),
        }
    }
    guarded(
        send(
            socket,
            subscribe_frame(&credentials.vehicle_id, READ_RVMS),
            timings.write,
        ),
        cancel,
        revocation,
        credentials.epoch,
    )
    .await?;
    status(state, generation, ConnectionState::Connected, None).await;
    let mut heartbeat =
        tokio::time::interval_at(Instant::now() + timings.heartbeat, timings.heartbeat);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_frame = Instant::now();
    loop {
        let frame = guarded(
            async {
                tokio::select! {
                    frame = receive(socket) => frame.map(Some),
                    _ = heartbeat.tick() => Ok(None),
                }
            },
            cancel,
            revocation,
            credentials.epoch,
        )
        .await?;
        let Some(frame) = frame else {
            if last_frame.elapsed() >= timings.idle {
                return Err(Failure::Retry("telemetry_heartbeat_timeout"));
            }
            guarded(
                send(socket, json!({"type":"ping"}), timings.write),
                cancel,
                revocation,
                credentials.epoch,
            )
            .await?;
            continue;
        };
        last_frame = Instant::now();
        let kind = frame
            .get("type")
            .and_then(Value::as_str)
            .ok_or(Failure::Fatal("telemetry_protocol"))?;
        if matches!(kind, "next" | "error" | "complete")
            && frame.get("id").and_then(Value::as_str) != Some(SUBSCRIPTION_ID)
        {
            continue;
        }
        match kind {
            "ping" => {
                guarded(
                    send(socket, pong(&frame), timings.write),
                    cancel,
                    revocation,
                    credentials.epoch,
                )
                .await?;
            }
            "pong" => {}
            "complete" => return Err(Failure::Retry("telemetry_completed")),
            "error" => return Err(graphql_error(&frame["payload"])),
            "next" => {
                if frame.pointer("/payload/errors").is_some() {
                    return Err(graphql_error(&frame["payload"]["errors"]));
                }
                let payload = frame
                    .pointer("/payload/data/parallaxMessages")
                    .ok_or(Failure::Fatal("telemetry_protocol"))?;
                let rvm = payload
                    .get("rvm")
                    .and_then(Value::as_str)
                    .ok_or(Failure::Fatal("telemetry_protocol"))?;
                let mut state = state.lock().await;
                // Recheck while holding publication authority: an old account or
                // vehicle worker cannot publish after revocation/replacement.
                if !credentials.is_current() {
                    return Err(Failure::Revoked);
                }
                if state.generation != generation {
                    return Err(Failure::Canceled);
                }
                // Request admission is also response admission. Unrequested
                // topics can contain phone/key enrollment credentials, so they
                // must never reach generic raw-topic retention or serialization.
                if !READ_RVMS.contains(&rvm) {
                    state.error_code = Some("telemetry_topic_not_requested");
                    continue;
                }
                let bytes = payload
                    .get("payload")
                    .and_then(Value::as_str)
                    .ok_or(Failure::Fatal("telemetry_protocol"))?;
                let timestamp = payload
                    .get("timestamp")
                    .and_then(Value::as_i64)
                    .ok_or(Failure::Fatal("telemetry_protocol"))?;
                if let Some(telemetry) = &mut state.telemetry {
                    state.error_code = telemetry
                        .apply(rvm, bytes, timestamp)
                        .err()
                        .map(|_| "telemetry_payload_invalid");
                }
            }
            _ => return Err(Failure::Fatal("telemetry_protocol")),
        }
    }
}

fn pong(frame: &Value) -> Value {
    let mut pong = json!({"type":"pong"});
    if let Some(payload) = frame.get("payload") {
        pong["payload"] = payload.clone();
    }
    pong
}

fn graphql_error(errors: &Value) -> Failure {
    if errors.as_array().is_some_and(|errors| {
        errors.iter().any(|error| {
            matches!(
                error.pointer("/extensions/code").and_then(Value::as_str),
                Some("UNAUTHENTICATED" | "FORBIDDEN")
            )
        })
    }) {
        Failure::Authentication
    } else {
        Failure::Fatal("telemetry_subscription_rejected")
    }
}

const SUBSCRIPTION_ID: &str = "owner-telemetry";
const QUERY: &str = "subscription ParallaxMessages($vehicleId: String!, $rvms: [String!]) { parallaxMessages(vehicleId: $vehicleId, rvms: $rvms) { payload timestamp rvm } }";

fn subscribe_frame(vehicle_id: &str, rvms: &[&str]) -> Value {
    json!({"id":SUBSCRIPTION_ID,"type":"subscribe","payload":{"operationName":"ParallaxMessages","query":QUERY,"variables":{"vehicleId":vehicle_id,"rvms":rvms}}})
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn delayed_stop_cannot_cancel_a_newer_account_stream() {
        let service = std::sync::Arc::new(super::TelemetryService::new());
        let gate = service.gate.lock().await;
        let older = std::sync::Arc::clone(&service);
        let pending = tokio::spawn(async move { older.stop_at_epoch(42).await });
        {
            let mut state = service.state.lock().await;
            state.authorization_epoch = Some(43);
            state.connection = super::ConnectionState::Connecting;
        }
        drop(gate);
        assert_eq!(
            pending.await.unwrap().unwrap_err().code(),
            "session_revoked"
        );
        assert_eq!(service.snapshot().await.authorization_epoch, Some(43));
        assert_eq!(
            service.snapshot().await.state,
            super::ConnectionState::Connecting
        );
        service.stop_at_epoch(43).await.unwrap();
        assert_eq!(
            service.snapshot().await.state,
            super::ConnectionState::Stopped
        );
    }
    use super::*;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::{oneshot, watch};
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
    use tokio_tungstenite::{WebSocketStream, accept_hdr_async};
    use zeroize::Zeroizing;

    const BATTERY: &str = "CgkJAAAAAABAUEA=";
    const TIMESTAMP: i64 = 1_780_000_000_000;

    fn credentials(vehicle: &str) -> (SubscriptionCredentials, watch::Sender<u64>) {
        let (epoch, revocation) = watch::channel(0);
        (
            SubscriptionCredentials {
                vehicle_id: vehicle.into(),
                csrf: Zeroizing::new("mock-csrf".into()),
                app_session: Zeroizing::new("mock-app".into()),
                user_session: Zeroizing::new("mock-user".into()),
                epoch: 0,
                revocation,
            },
            epoch,
        )
    }

    async fn listener() -> (TcpListener, String) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}/graphql", listener.local_addr().unwrap());
        (listener, endpoint)
    }

    // The dependency's handshake callback requires its concrete ErrorResponse.
    #[allow(clippy::result_large_err)]
    async fn accept(listener: &TcpListener) -> WebSocketStream<TcpStream> {
        let (stream, _) = listener.accept().await.unwrap();
        accept_hdr_async(stream, |request: &Request, mut response: Response| {
            assert_eq!(request.headers()["a-sess"], "mock-app");
            assert_eq!(request.headers()["u-sess"], "mock-user");
            assert_eq!(request.headers()["csrf-token"], "mock-csrf");
            assert_eq!(
                request.headers()["apollographql-client-name"],
                "com.rivian.android.consumer"
            );
            assert_eq!(request.headers()["sec-websocket-protocol"], PROTOCOL);
            response
                .headers_mut()
                .insert("sec-websocket-protocol", HeaderValue::from_static(PROTOCOL));
            Ok(response)
        })
        .await
        .unwrap()
    }

    async fn read_json(socket: &mut WebSocketStream<TcpStream>) -> Value {
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(2), socket.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            match frame {
                Message::Text(text) => return serde_json::from_str(&text).unwrap(),
                Message::Ping(payload) => socket.send(Message::Pong(payload)).await.unwrap(),
                _ => panic!("Expected a protocol text frame"),
            }
        }
    }

    async fn send_json(socket: &mut WebSocketStream<TcpStream>, value: Value) {
        socket
            .send(Message::Text(value.to_string().into()))
            .await
            .unwrap();
    }

    async fn acknowledge(socket: &mut WebSocketStream<TcpStream>, vehicle: &str) {
        assert_eq!(read_json(socket).await, json!({"type":"connection_init"}));
        send_json(socket, json!({"type":"connection_ack"})).await;
        let subscription = read_json(socket).await;
        assert_eq!(subscription, subscribe_frame(vehicle, READ_RVMS));
    }

    fn battery_frame() -> Value {
        json!({"id":SUBSCRIPTION_ID,"type":"next","payload":{"data":{"parallaxMessages":{"rvm":"energy.high_voltage.battery_state","payload":BATTERY,"timestamp":TIMESTAMP}}}})
    }

    async fn wait_snapshot(
        service: &TelemetryService,
        predicate: impl Fn(&TelemetrySnapshot) -> bool,
    ) -> TelemetrySnapshot {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let snapshot = service.snapshot().await;
                if predicate(&snapshot) {
                    return snapshot;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("Expected native snapshot state")
    }

    #[test]
    fn subscription_uses_reviewed_document_and_vehicle_id_casing() {
        let frame = subscribe_frame("01-owned", &["energy.high_voltage.battery_state"]);
        assert_eq!(frame["type"], "subscribe");
        assert_eq!(frame["payload"]["operationName"], "ParallaxMessages");
        assert_eq!(frame["payload"]["variables"]["vehicleId"], "01-owned");
        assert!(frame["payload"]["variables"].get("vehicleID").is_none());
        assert!(
            frame["payload"]["query"]
                .as_str()
                .unwrap()
                .starts_with("subscription ")
        );
        assert_eq!(
            frame["payload"]["variables"]["rvms"],
            json!(["energy.high_voltage.battery_state"])
        );
    }

    #[tokio::test]
    async fn authenticates_acknowledges_reads_and_answers_both_ping_types() {
        let (listener, endpoint) = listener().await;
        let (finished, done) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = accept(&listener).await;
            assert_eq!(
                read_json(&mut socket).await,
                json!({"type":"connection_init"})
            );
            send_json(&mut socket, json!({"type":"ping","payload":{"probe":1}})).await;
            assert_eq!(
                read_json(&mut socket).await,
                json!({"type":"pong","payload":{"probe":1}})
            );
            send_json(&mut socket, json!({"type":"connection_ack"})).await;
            assert_eq!(
                read_json(&mut socket).await,
                subscribe_frame("01-owned", READ_RVMS)
            );
            send_json(&mut socket, battery_frame()).await;
            socket.send(Message::Ping(vec![7, 8].into())).await.unwrap();
            assert_eq!(
                socket.next().await.unwrap().unwrap(),
                Message::Pong(vec![7, 8].into())
            );
            send_json(&mut socket, json!({"type":"ping"})).await;
            assert_eq!(read_json(&mut socket).await, json!({"type":"pong"}));
            finished.send(()).unwrap();
            assert_eq!(
                read_json(&mut socket).await,
                json!({"id":SUBSCRIPTION_ID,"type":"complete"})
            );
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, _epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        let snapshot = wait_snapshot(&service, |snapshot| {
            snapshot.readings["battery_percent"] == 65.0
        })
        .await;
        done.await.unwrap();
        assert_eq!(snapshot.state, ConnectionState::Connected);
        assert_eq!(snapshot.compatibility, "unverified");
        assert_eq!(snapshot.readings["id"], "01-owned");
        assert!(
            !serde_json::to_string(&snapshot)
                .unwrap()
                .contains("mock-user")
        );
        service.stop().await;
        assert_eq!(service.snapshot().await.state, ConnectionState::Stopped);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn reconnects_and_resubscribes_to_the_same_owned_vehicle() {
        let (listener, endpoint) = listener().await;
        let (finished, done) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut first = accept(&listener).await;
            acknowledge(&mut first, "01-owned").await;
            first.close(None).await.unwrap();
            let mut second = accept(&listener).await;
            acknowledge(&mut second, "01-owned").await;
            send_json(&mut second, battery_frame()).await;
            finished.send(()).unwrap();
            assert_eq!(read_json(&mut second).await["type"], "complete");
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, _epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        assert_eq!(
            wait_snapshot(&service, |snapshot| snapshot.readings["battery_percent"]
                == 65.0)
            .await
            .state,
            ConnectionState::Connected
        );
        done.await.unwrap();
        service.stop().await;
        server.await.unwrap();
    }

    #[tokio::test]
    async fn revoked_credentials_never_open_a_socket() {
        let (listener, endpoint) = listener().await;
        let service = TelemetryService::test_new(endpoint);
        let (credentials, epoch) = credentials("01-owned");
        epoch.send(1).unwrap();
        assert_eq!(
            service.start(credentials).await.unwrap_err().code(),
            "session_revoked"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn revocation_clears_readings_and_stops_reconnect() {
        let (listener, endpoint) = listener().await;
        let (finished, done) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = accept(&listener).await;
            acknowledge(&mut socket, "01-owned").await;
            send_json(&mut socket, battery_frame()).await;
            finished.send(()).unwrap();
            let _ = tokio::time::timeout(Duration::from_secs(1), socket.next()).await;
            assert!(
                tokio::time::timeout(Duration::from_millis(120), listener.accept())
                    .await
                    .is_err()
            );
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        done.await.unwrap();
        wait_snapshot(&service, |snapshot| {
            snapshot.readings["battery_percent"] == 65.0
        })
        .await;
        epoch.send(1).unwrap();
        let snapshot = service.snapshot().await;
        assert!(snapshot.vehicle_id.is_none());
        assert!(snapshot.readings.is_null());
        assert!(snapshot.raw_topics.is_empty());
        assert_eq!(snapshot.state, ConnectionState::Stopped);
        assert_eq!(snapshot.error_code, Some("session_revoked"));
        assert!(snapshot.authorization_epoch.is_none());
        service.stop().await;
        server.await.unwrap();
    }

    #[tokio::test]
    async fn changing_vehicle_clears_previous_readings_and_completes_previous_worker() {
        let (listener, endpoint) = listener().await;
        let (received, receive) = oneshot::channel();
        let (replaced, replacement) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut first = accept(&listener).await;
            acknowledge(&mut first, "01-first").await;
            send_json(&mut first, battery_frame()).await;
            received.send(()).unwrap();
            assert_eq!(read_json(&mut first).await["type"], "complete");
            let mut second = accept(&listener).await;
            acknowledge(&mut second, "01-second").await;
            replaced.send(()).unwrap();
            assert_eq!(read_json(&mut second).await["type"], "complete");
        });
        let service = TelemetryService::test_new(endpoint);
        let (first, _first_epoch) = credentials("01-first");
        service.start(first).await.unwrap();
        receive.await.unwrap();
        wait_snapshot(&service, |snapshot| {
            snapshot.readings["battery_percent"] == 65.0
        })
        .await;
        let (second, _second_epoch) = credentials("01-second");
        service.start(second).await.unwrap();
        replacement.await.unwrap();
        let snapshot = service.snapshot().await;
        assert_eq!(snapshot.vehicle_id.as_deref(), Some("01-second"));
        assert!(snapshot.readings["battery_percent"].is_null());
        assert!(snapshot.raw_topics.is_empty());
        service.stop().await;
        server.await.unwrap();
    }

    #[tokio::test]
    async fn acknowledgement_timeout_retries_and_can_be_stopped() {
        let (listener, endpoint) = listener().await;
        let server = tokio::spawn(async move {
            let mut first = accept(&listener).await;
            assert_eq!(read_json(&mut first).await["type"], "connection_init");
            let mut second = accept(&listener).await;
            acknowledge(&mut second, "01-owned").await;
            send_json(&mut second, battery_frame()).await;
            assert_eq!(read_json(&mut second).await["type"], "complete");
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, _epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        wait_snapshot(&service, |snapshot| {
            snapshot.readings["battery_percent"] == 65.0
        })
        .await;
        service.stop().await;
        server.await.unwrap();
    }

    #[tokio::test]
    async fn handshake_timeout_retries_without_waiting_for_the_stalled_socket() {
        use tokio::io::AsyncReadExt;
        let (listener, endpoint) = listener().await;
        let server = tokio::spawn(async move {
            let (mut stalled, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            assert!(stalled.read(&mut request).await.unwrap() > 0);
            let mut second = accept(&listener).await;
            acknowledge(&mut second, "01-owned").await;
            send_json(&mut second, battery_frame()).await;
            assert_eq!(read_json(&mut second).await["type"], "complete");
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, _epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        wait_snapshot(&service, |snapshot| {
            snapshot.readings["battery_percent"] == 65.0
        })
        .await;
        service.stop().await;
        server.await.unwrap();
    }

    #[tokio::test]
    async fn revocation_during_handshake_drops_socket_and_suppresses_all_results() {
        use tokio::io::AsyncReadExt;
        let (listener, endpoint) = listener().await;
        let (received, request_received) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stalled, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            assert!(stalled.read(&mut request).await.unwrap() > 0);
            received.send(()).unwrap();
            let mut remaining = Vec::new();
            tokio::time::timeout(Duration::from_secs(1), stalled.read_to_end(&mut remaining))
                .await
                .unwrap()
                .unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(120), listener.accept())
                    .await
                    .is_err()
            );
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        request_received.await.unwrap();
        epoch.send(1).unwrap();
        let snapshot = service.snapshot().await;
        assert!(snapshot.vehicle_id.is_none());
        assert!(snapshot.readings.is_null());
        service.stop().await;
        server.await.unwrap();
    }

    #[tokio::test]
    async fn revocation_during_acknowledgement_cancels_without_subscribing() {
        let (listener, endpoint) = listener().await;
        let (initialized, init) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = accept(&listener).await;
            assert_eq!(read_json(&mut socket).await["type"], "connection_init");
            initialized.send(()).unwrap();
            assert_eq!(read_json(&mut socket).await["type"], "complete");
            assert!(
                tokio::time::timeout(Duration::from_millis(120), listener.accept())
                    .await
                    .is_err()
            );
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        init.await.unwrap();
        epoch.send(1).unwrap();
        let snapshot = service.snapshot().await;
        assert!(snapshot.vehicle_id.is_none());
        assert!(snapshot.readings.is_null());
        service.stop().await;
        server.await.unwrap();
    }

    #[tokio::test]
    async fn stop_cancels_backoff_before_another_connection() {
        let (listener, endpoint) = listener().await;
        let server = tokio::spawn(async move {
            let mut socket = accept(&listener).await;
            acknowledge(&mut socket, "01-owned").await;
            socket.close(None).await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(150), listener.accept())
                    .await
                    .is_err()
            );
        });
        let mut service = TelemetryService::test_new(endpoint);
        service.timings.retry_initial = Duration::from_secs(1);
        let (credentials, _epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        wait_snapshot(&service, |snapshot| {
            snapshot.state == ConnectionState::Reconnecting
        })
        .await;
        tokio::time::timeout(Duration::from_millis(200), service.stop())
            .await
            .unwrap();
        assert_eq!(service.snapshot().await.state, ConnectionState::Stopped);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn rapid_disconnects_exhaust_a_bounded_retry_budget() {
        let (listener, endpoint) = listener().await;
        let server = tokio::spawn(async move {
            for _ in 0..=MAX_RECONNECT_ATTEMPTS {
                let mut socket = accept(&listener).await;
                assert_eq!(read_json(&mut socket).await["type"], "connection_init");
                socket.close(None).await.unwrap();
            }
            assert!(
                tokio::time::timeout(Duration::from_millis(150), listener.accept())
                    .await
                    .is_err()
            );
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, _epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        let snapshot = wait_snapshot(&service, |snapshot| {
            snapshot.state == ConnectionState::Error
        })
        .await;
        assert_eq!(snapshot.error_code, Some("telemetry_reconnect_exhausted"));
        service.stop().await;
        server.await.unwrap();
    }

    #[tokio::test]
    async fn authentication_failure_stops_without_reconnecting_or_exposing_upstream_text() {
        let (listener, endpoint) = listener().await;
        let server = tokio::spawn(async move {
            let mut socket = accept(&listener).await;
            acknowledge(&mut socket, "01-owned").await;
            send_json(&mut socket, json!({"id":SUBSCRIPTION_ID,"type":"error","payload":[{"message":"private upstream token mock-user","extensions":{"code":"UNAUTHENTICATED"}}]})).await;
            assert!(
                tokio::time::timeout(Duration::from_millis(200), listener.accept())
                    .await
                    .is_err()
            );
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, _epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        let snapshot = wait_snapshot(&service, |snapshot| {
            snapshot.state == ConnectionState::AuthenticationRequired
        })
        .await;
        assert_eq!(snapshot.error_code, Some("account_auth_required"));
        assert!(
            !serde_json::to_string(&snapshot)
                .unwrap()
                .contains("mock-user")
        );
        service.stop().await;
        server.await.unwrap();
    }

    #[tokio::test]
    async fn snapshot_retains_native_failure_epoch_without_serializing_authority() {
        let (listener, endpoint) = listener().await;
        let server = tokio::spawn(async move {
            let mut socket = accept(&listener).await;
            acknowledge(&mut socket, "01-owned").await;
            send_json(&mut socket, json!({"id":SUBSCRIPTION_ID,"type":"error","payload":[{"extensions":{"code":"UNAUTHENTICATED"}}]})).await;
        });
        let service = TelemetryService::test_new(endpoint);
        let (mut credentials, epoch) = credentials("01-owned");
        credentials.epoch = 42;
        epoch.send(42).unwrap();
        credentials.revocation = epoch.subscribe();
        service.start(credentials).await.unwrap();
        let snapshot = wait_snapshot(&service, |snapshot| {
            snapshot.error_code == Some("account_auth_required")
        })
        .await;
        assert_eq!(snapshot.authorization_epoch, Some(42));
        assert!(
            serde_json::to_value(&snapshot)
                .unwrap()
                .get("authorization_epoch")
                .is_none()
        );
        epoch.send_replace(43);
        // A later sign-in must not relabel the old failure as its own authority.
        assert_eq!(service.snapshot().await.authorization_epoch, Some(42));
        service.stop().await;
        assert!(service.snapshot().await.authorization_epoch.is_none());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn oversized_and_invalid_frames_fail_closed() {
        for frame in [
            Message::Text("x".repeat(MAX_FRAME_BYTES + 1).into()),
            Message::Text("{bad-json".into()),
            Message::Binary(vec![1, 2].into()),
        ] {
            let (listener, endpoint) = listener().await;
            let server = tokio::spawn(async move {
                let mut socket = accept(&listener).await;
                acknowledge(&mut socket, "01-owned").await;
                socket.send(frame).await.unwrap();
                assert!(
                    tokio::time::timeout(Duration::from_millis(150), listener.accept())
                        .await
                        .is_err()
                );
            });
            let service = TelemetryService::test_new(endpoint);
            let (credentials, _epoch) = credentials("01-owned");
            service.start(credentials).await.unwrap();
            let snapshot = wait_snapshot(&service, |snapshot| {
                snapshot.state == ConnectionState::Error
            })
            .await;
            assert!(snapshot.readings["battery_percent"].is_null());
            service.stop().await;
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn malformed_payload_and_unrequested_topics_preserve_known_reading() {
        let (listener, endpoint) = listener().await;
        let (finished, done) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = accept(&listener).await;
            acknowledge(&mut socket, "01-owned").await;
            send_json(&mut socket, battery_frame()).await;
            let mut malformed = battery_frame();
            malformed["payload"]["data"]["parallaxMessages"]["payload"] = json!("!");
            malformed["payload"]["data"]["parallaxMessages"]["timestamp"] = json!(TIMESTAMP + 1);
            send_json(&mut socket, malformed).await;
            let mut unknown = battery_frame();
            unknown["payload"]["data"]["parallaxMessages"]["rvm"] = json!("future.unknown.state");
            send_json(&mut socket, unknown).await;
            finished.send(()).unwrap();
            assert_eq!(read_json(&mut socket).await["type"], "complete");
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, _epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        done.await.unwrap();
        let snapshot = wait_snapshot(&service, |snapshot| {
            snapshot.error_code == Some("telemetry_topic_not_requested")
        })
        .await;
        assert_eq!(snapshot.readings["battery_percent"], 65.0);
        assert_eq!(snapshot.state, ConnectionState::Connected);
        assert!(snapshot.raw_topics.is_empty());
        service.stop().await;
        server.await.unwrap();
    }

    #[tokio::test]
    async fn unrequested_device_credentials_never_enter_browser_snapshots() {
        const PRIVATE_TOPIC: &str = "device_table.vas_keyper.devices";
        const PRIVATE_PAYLOAD: &str = "Cg5tb2NrLWtleS10b2tlbg==";
        let (listener, endpoint) = listener().await;
        let server = tokio::spawn(async move {
            let mut socket = accept(&listener).await;
            acknowledge(&mut socket, "01-owned").await;
            send_json(&mut socket, battery_frame()).await;
            let mut unexpected = battery_frame();
            unexpected["payload"]["data"]["parallaxMessages"]["rvm"] = json!(PRIVATE_TOPIC);
            unexpected["payload"]["data"]["parallaxMessages"]["payload"] = json!(PRIVATE_PAYLOAD);
            send_json(&mut socket, unexpected).await;
            assert_eq!(read_json(&mut socket).await["type"], "complete");
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, _epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        let snapshot = wait_snapshot(&service, |snapshot| {
            snapshot.error_code == Some("telemetry_topic_not_requested")
                || snapshot
                    .raw_topics
                    .iter()
                    .any(|entry| entry["rvm"] == PRIVATE_TOPIC)
        })
        .await;
        assert!(
            snapshot.raw_topics.is_empty(),
            "Unrequested native payload must not cross the browser boundary"
        );
        let serialized = serde_json::to_string(&snapshot).unwrap();
        assert!(!serialized.contains(PRIVATE_PAYLOAD));
        assert!(!serialized.contains(PRIVATE_TOPIC));
        assert_eq!(snapshot.readings["battery_percent"], 65.0);
        service.stop().await;
        server.await.unwrap();
    }

    #[tokio::test]
    async fn redirect_cannot_forward_native_credentials_to_another_destination() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (listener, endpoint) = listener().await;
        let (target, target_endpoint) = self::listener().await;
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0; 4096];
            let _ = socket.read(&mut buffer).await.unwrap();
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: {target_endpoint}\r\nContent-Length: 0\r\n\r\n"
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(200), target.accept())
                    .await
                    .is_err()
            );
        });
        let service = TelemetryService::test_new(endpoint);
        let (credentials, _epoch) = credentials("01-owned");
        service.start(credentials).await.unwrap();
        wait_snapshot(&service, |snapshot| {
            snapshot.state == ConnectionState::Error
        })
        .await;
        service.stop().await;
        server.await.unwrap();
    }
}
