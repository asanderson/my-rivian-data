#![forbid(unsafe_code)]

//! Loopback-only application host with an isolated native Rivian session per launcher.

mod confirmations;

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;

const MAX_BODY_BYTES: usize = 32 * 1024;
const CSP: &str = "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

#[derive(RustEmbed)]
#[folder = "../../ui/dist/"]
struct Assets;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    demo: bool,
    remember_enabled: bool,
    shutdown: tokio::sync::watch::Sender<bool>,
    authority: String,
    origin: String,
    cookie_name: String,
    session_idle: Duration,
    session_lifetime: Duration,
    auth: Mutex<AuthState>,
}

struct AuthState {
    bootstrap: Option<Bootstrap>,
    session: Option<Session>,
}

struct Bootstrap {
    secret: String,
    expires_at: Instant,
}

struct Session {
    secret: String,
    csrf: String,
    capability: String,
    client: Option<Arc<rivian_api::LiveClient>>,
    telemetry: Arc<rivian_api::telemetry::TelemetryService>,
    confirmations: confirmations::Confirmations,
    created_at: Instant,
    last_used: Instant,
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(client) = &self.client {
            client.revoke();
        }
    }
}

impl AppState {
    /// Returns a single-use launcher capability. Never log it in ordinary operation.
    pub fn new(port: u16) -> std::io::Result<(Self, String)> {
        Self::with_mode_and_lifetimes(
            port,
            false,
            true,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(8 * 3600),
        )
    }

    /// Session-only mode never reads or writes an OS credential entry.
    pub fn new_ephemeral(port: u16) -> std::io::Result<(Self, String)> {
        Self::with_mode_and_lifetimes(
            port,
            false,
            false,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(8 * 3600),
        )
    }

    pub fn new_demo(port: u16) -> std::io::Result<(Self, String)> {
        Self::with_lifetimes(
            port,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(8 * 3600),
        )
    }

    /// Configurable demo lifetimes for deterministic boundary tests.
    pub fn with_lifetimes(
        port: u16,
        bootstrap_lifetime: Duration,
        session_idle: Duration,
        session_lifetime: Duration,
    ) -> std::io::Result<(Self, String)> {
        Self::with_mode_and_lifetimes(
            port,
            true,
            false,
            bootstrap_lifetime,
            session_idle,
            session_lifetime,
        )
    }

    fn with_mode_and_lifetimes(
        port: u16,
        demo: bool,
        remember_enabled: bool,
        bootstrap_lifetime: Duration,
        session_idle: Duration,
        session_lifetime: Duration,
    ) -> std::io::Result<(Self, String)> {
        let secret = random_secret()?;
        let authority = format!("127.0.0.1:{port}");
        let cookie_name = format!("my_rivian_data_{port}_{}", &random_secret()?[..16]);
        Ok((
            Self {
                inner: Arc::new(Inner {
                    demo,
                    remember_enabled,
                    shutdown: tokio::sync::watch::channel(false).0,
                    origin: format!("http://{authority}"),
                    authority,
                    cookie_name,
                    session_idle,
                    session_lifetime,
                    auth: Mutex::new(AuthState {
                        bootstrap: Some(Bootstrap {
                            secret: secret.clone(),
                            expires_at: Instant::now() + bootstrap_lifetime,
                        }),
                        session: None,
                    }),
                }),
            },
            secret,
        ))
    }

    pub fn origin(&self) -> &str {
        &self.inner.origin
    }

    pub fn shutdown_receiver(&self) -> tokio::sync::watch::Receiver<bool> {
        self.inner.shutdown.subscribe()
    }

    fn telemetry(
        &self,
        headers: &HeaderMap,
        csrf: bool,
    ) -> Result<Arc<rivian_api::telemetry::TelemetryService>, ApiError> {
        self.live_client(headers, csrf)?;
        let auth = self.inner.auth.lock().map_err(|_| ApiError::internal())?;
        auth.session
            .as_ref()
            .map(|session| session.telemetry.clone())
            .ok_or_else(ApiError::unauthorized)
    }

    fn mode(&self) -> &'static str {
        if self.inner.demo { "demo" } else { "live" }
    }

    /// Release native credentials on expiry even when the browser stops requesting data.
    pub fn expire_sessions(&self) {
        if let Ok(mut auth) = self.inner.auth.lock() {
            let now = Instant::now();
            if auth.session.as_ref().is_some_and(|session| {
                now.duration_since(session.created_at) >= self.inner.session_lifetime
                    || now.duration_since(session.last_used) >= self.inner.session_idle
            }) {
                auth.session = None;
            }
        }
    }

    fn live_client(
        &self,
        headers: &HeaderMap,
        csrf: bool,
    ) -> Result<Arc<rivian_api::LiveClient>, ApiError> {
        self.session_info(headers, csrf)?;
        if self.inner.demo {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "demo_mode",
                message: "Rivian account access is unavailable in sample mode. Restart without --demo to sign in.",
            });
        }
        let auth = self.inner.auth.lock().map_err(|_| ApiError::internal())?;
        auth.session
            .as_ref()
            .and_then(|s| s.client.clone())
            .ok_or_else(ApiError::unauthorized)
    }

    fn session_info(&self, headers: &HeaderMap, require_csrf: bool) -> Result<Value, ApiError> {
        let provided =
            cookie_value(headers, &self.inner.cookie_name).ok_or_else(ApiError::unauthorized)?;
        let mut auth = self.inner.auth.lock().map_err(|_| ApiError::internal())?;
        let session = auth.session.as_mut().ok_or_else(ApiError::unauthorized)?;
        let now = Instant::now();
        if now.duration_since(session.created_at) >= self.inner.session_lifetime
            || now.duration_since(session.last_used) >= self.inner.session_idle
        {
            auth.session = None;
            return Err(ApiError::unauthorized());
        }
        // A localhost cookie is not isolated by port. Require an independent,
        // origin-scoped tab capability on ALL protected requests, including recovery.
        let capability =
            single_header(headers, "x-app-capability").ok_or_else(ApiError::unauthorized)?;
        if !secret_matches(&session.capability, capability)
            || !secret_matches(&session.secret, provided)
        {
            return Err(ApiError::unauthorized());
        }
        if require_csrf {
            let csrf = single_header(headers, "x-csrf-token").ok_or_else(ApiError::forbidden)?;
            if !secret_matches(&session.csrf, csrf) {
                return Err(ApiError::forbidden());
            }
        }
        session.last_used = now;
        Ok(json!({
            "mode": self.mode(),
            "csrf_token": session.csrf,
            "expires_in_seconds": self.inner.session_lifetime.saturating_sub(now.duration_since(session.created_at)).as_secs(),
        }))
    }
}

fn random_secret() -> std::io::Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn secret_matches(expected: &str, actual: &str) -> bool {
    expected.len() == actual.len() && bool::from(expected.as_bytes().ct_eq(actual.as_bytes()))
}

fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(value)
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut result = None;
    for value in headers.get_all(header::COOKIE) {
        for part in value.to_str().ok()?.split(';') {
            let Some((key, val)) = part.trim().split_once('=') else {
                continue;
            };
            if key == name {
                if result.is_some() {
                    return None;
                }
                result = Some(val);
            }
        }
    }
    result
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}

impl ApiError {
    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "session_required",
            message: "Launch the application again to establish a local session.",
        }
    }
    fn forbidden() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "request_forbidden",
            message: "The request failed local security checks.",
        }
    }
    fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: "The application could not complete this request.",
        }
    }
    fn invalid() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_operation",
            message: "The operation or its variables are invalid or unavailable.",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error": {"code": self.code, "message": self.message}})),
        )
            .into_response()
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/bootstrap", post(bootstrap))
        .route("/api/session", get(session))
        .route("/api/logout", post(logout))
        .route("/api/account", get(account))
        .route("/api/account/vault", get(account_vault))
        .route("/api/account/remember", post(account_remember))
        .route("/api/account/otp/resend", post(account_resend))
        .route("/api/telemetry", get(telemetry_snapshot))
        .route("/api/telemetry/start", post(telemetry_start))
        .route("/api/telemetry/stop", post(telemetry_stop))
        .route("/api/catalog/admit", post(admit_document))
        .route("/api/commands/prepare", post(prepare_command))
        .route("/api/commands/execute", post(execute_command))
        .route("/api/application/quit", post(quit_application))
        .route("/api/account/login", post(account_login))
        .route("/api/account/otp", post(account_otp))
        .route("/api/account/logout", post(account_logout))
        .route("/api/catalog", get(catalog))
        .route("/api/vehicles", get(vehicles))
        .route("/api/validate", post(validate))
        .route("/api/execute", post(execute))
        .fallback(static_asset)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), boundary))
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    response
}

async fn boundary(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let headers = request.headers();
    if single_header(headers, "host") != Some(state.inner.authority.as_str()) {
        return ApiError::forbidden().into_response();
    }
    if headers.contains_key(header::ORIGIN)
        && single_header(headers, "origin") != Some(state.inner.origin.as_str())
    {
        return ApiError::forbidden().into_response();
    }
    if headers.contains_key("sec-fetch-site")
        && !matches!(
            single_header(headers, "sec-fetch-site"),
            Some("same-origin" | "none")
        )
    {
        return ApiError::forbidden().into_response();
    }
    let unsafe_method = !matches!(*request.method(), Method::GET | Method::HEAD);
    if unsafe_method && single_header(headers, "origin") != Some(state.inner.origin.as_str()) {
        return ApiError::forbidden().into_response();
    }
    let path = request.uri().path();
    if path.starts_with("/api/")
        && path != "/api/bootstrap"
        && path != "/api/health"
        && let Err(error) = state.session_info(headers, unsafe_method)
    {
        return error.into_response();
    }
    // Capture account authority BEFORE body extraction or any queued async work.
    if path.starts_with("/api/") && path != "/api/bootstrap" && path != "/api/health" {
        let epoch = if state.inner.demo {
            None
        } else {
            state
                .live_client(headers, unsafe_method)
                .ok()
                .map(|client| client.authorization_epoch())
        };
        request.extensions_mut().insert(NativeEpoch(epoch));
    }
    next.run(request).await
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({"status": "ok", "mode": state.mode(), "version": env!("CARGO_PKG_VERSION")}))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapRequest {
    token: String,
}

async fn bootstrap(
    State(state): State<AppState>,
    Json(body): Json<BootstrapRequest>,
) -> Result<Response, ApiError> {
    let (response, client) = {
        let mut auth = state.inner.auth.lock().map_err(|_| ApiError::internal())?;
        let bootstrap = auth.bootstrap.as_ref().ok_or_else(ApiError::unauthorized)?;
        if Instant::now() >= bootstrap.expires_at {
            auth.bootstrap = None;
            return Err(ApiError::unauthorized());
        }
        if !secret_matches(&bootstrap.secret, &body.token) {
            return Err(ApiError::unauthorized());
        }
        let client = if state.inner.demo {
            None
        } else {
            Some(Arc::new(
                if state.inner.remember_enabled {
                    rivian_api::LiveClient::new()
                } else {
                    rivian_api::LiveClient::memory_only()
                }
                .map_err(|_| ApiError::internal())?,
            ))
        };
        let now = Instant::now();
        let session = Session {
            secret: random_secret().map_err(|_| ApiError::internal())?,
            csrf: random_secret().map_err(|_| ApiError::internal())?,
            capability: random_secret().map_err(|_| ApiError::internal())?,
            client: client.clone(),
            telemetry: Arc::new(rivian_api::telemetry::TelemetryService::new()),
            confirmations: confirmations::Confirmations::default(),
            created_at: now,
            last_used: now,
        };
        let cookie = format!(
            "{}={}; HttpOnly; SameSite=Strict; Path=/",
            state.inner.cookie_name, session.secret
        );
        let mut response = Json(json!({"csrf_token":session.csrf,"app_capability":session.capability,"mode":state.mode(),"expires_in_seconds":state.inner.session_lifetime.as_secs()})).into_response();
        response.headers_mut().insert(
            header::SET_COOKIE,
            HeaderValue::from_str(&cookie).map_err(|_| ApiError::internal())?,
        );
        auth.bootstrap = None;
        auth.session = Some(session);
        (response, client)
    };
    // OS/network IO never holds the synchronous local-authority mutex. Failure
    // to restore leaves the owner able to use a protected session-only sign-in.
    if state.inner.remember_enabled
        && let Some(client) = client
    {
        let _ = client.restore_remembered().await;
    }
    Ok(response)
}

async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(state.session_info(&headers, false)?))
}

fn expire_cookie(state: &AppState, response: &mut Response) -> Result<(), ApiError> {
    let cookie = format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
        state.inner.cookie_name
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| ApiError::internal())?,
    );
    Ok(())
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    state.session_info(&headers, true)?;
    let client = {
        let mut auth = state.inner.auth.lock().map_err(|_| ApiError::internal())?;
        let session = auth.session.take().ok_or_else(ApiError::unauthorized)?;
        let client = session.client.clone();
        drop(session); // revoke before awaiting remote or OS work
        client
    };
    let forgotten = if let Some(client) = client {
        client.logout().await;
        let vault = client.vault_status().await;
        !state.inner.remember_enabled || (vault.error.is_none() && !vault.stored)
    } else {
        true
    };
    let mut response =
        Json(json!({"state":"ended","remembered_session_removed":forgotten})).into_response();
    expire_cookie(&state, &mut response)?;
    Ok(response)
}

async fn quit_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_body): Json<EmptyRequest>,
) -> Result<Response, ApiError> {
    state.session_info(&headers, true)?;
    state
        .inner
        .auth
        .lock()
        .map_err(|_| ApiError::internal())?
        .session = None;
    let mut response = Json(json!({"state":"stopping"})).into_response();
    expire_cookie(&state, &mut response)?;
    state.inner.shutdown.send_replace(true);
    Ok(response)
}

async fn catalog(State(state): State<AppState>) -> Json<Value> {
    if state.inner.demo {
        return Json(json!(rivian_core::catalog()));
    }
    let mut entries = rivian_core::inventory::enrich_catalog(&rivian_core::live::catalog());
    for entry in &mut entries {
        entry["upstream_operation"] = entry["operation_name"].clone();
        entry["upstream_document"] = entry["graphql_document"].clone();
    }
    Json(json!(entries))
}
async fn vehicles(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Extension(epoch): axum::Extension<NativeEpoch>,
) -> Result<Json<Value>, ApiError> {
    state.session_info(&headers, false)?;
    if state.inner.demo {
        return Ok(Json(json!(rivian_core::demo_vehicles())));
    }
    let client = state.live_client(&headers, false)?;
    let result = client
        .vehicles_at_epoch(account_epoch(epoch)?)
        .await
        .map_err(upstream_error)?;
    state.session_info(&headers, false)?;
    Ok(Json(result))
}

fn upstream_error(error: rivian_api::ApiError) -> ApiError {
    if error.code() == "session_revoked" {
        return account_changed();
    }
    let code = error.code();
    let status = match code {
        "account_auth_required" => StatusCode::UNAUTHORIZED,
        "rate_limited" | "login_throttled" => StatusCode::TOO_MANY_REQUESTS,
        "invalid_input"
        | "invalid_credentials"
        | "invalid_otp"
        | "mfa_required"
        | "mfa_expired"
        | "invalid_operation" => StatusCode::BAD_REQUEST,
        "session_revoked" => StatusCode::UNAUTHORIZED,
        "vault_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
        "vehicle_not_owned" | "resource_not_owned" => StatusCode::FORBIDDEN,
        _ => StatusCode::BAD_GATEWAY,
    };
    ApiError {
        status,
        code,
        message: error.message(),
    }
}

async fn account(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, false)?;
    let result = client.status().await;
    state.session_info(&headers, false)?;
    Ok(Json(json!(result)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    email: String,
    password: String,
    #[serde(default)]
    remember: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OtpRequest {
    code: String,
}

async fn account_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Extension(epoch): axum::Extension<NativeEpoch>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, true)?;
    let epoch = account_epoch(epoch)?;
    if client.authorization_epoch() != epoch {
        return Err(account_changed());
    }
    if body.email.len() > 254
        || !body.email.contains('@')
        || body.password.is_empty()
        || body.password.len() > 1024
    {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_credentials",
            message: "Enter your Rivian email address and password.",
        });
    }
    let result = client
        .login_with_remember(&body.email, &body.password, body.remember, epoch)
        .await
        .map_err(upstream_error)?;
    state.session_info(&headers, true)?;
    let mut response = json!(result);
    let vault = client.vault_status().await;
    response["remember_problem"] =
        json!(body.remember && (!vault.remembering || vault.error.is_some()));
    response["vault"] = json!(vault);
    Ok(Json(response))
}

async fn account_otp(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Extension(epoch): axum::Extension<NativeEpoch>,
    Json(body): Json<OtpRequest>,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, true)?;
    if !(6..=8).contains(&body.code.len()) || !body.code.bytes().all(|c| c.is_ascii_digit()) {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_otp",
            message: "Enter the verification code from Rivian.",
        });
    }
    let result = client
        .verify_otp_at_epoch(&body.code, account_epoch(epoch)?)
        .await
        .map_err(upstream_error)?;
    state.session_info(&headers, true)?;
    Ok(Json(json!(result)))
}

async fn account_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Extension(epoch): axum::Extension<NativeEpoch>,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, true)?;
    let telemetry = state.telemetry(&headers, true)?;
    let source_epoch = account_epoch(epoch)?;
    let outcome = client
        .logout_at_epoch(source_epoch)
        .await
        .map_err(upstream_error)?;
    // A retained failure from an earlier stream may have a different source
    // epoch. Never stop a newer stream; the final native epoch check decides
    // whether this account disconnect itself was superseded.
    if let Err(error) = telemetry.stop_at_epoch(source_epoch).await
        && error.code() != "session_revoked"
    {
        return Err(upstream_error(error));
    }
    state.session_info(&headers, true)?;
    state
        .inner
        .auth
        .lock()
        .map_err(|_| ApiError::internal())?
        .session
        .as_mut()
        .ok_or_else(ApiError::unauthorized)?
        .confirmations
        .forget_epoch(source_epoch);
    let vault = client.vault_status().await;
    if client.authorization_epoch() != outcome.epoch {
        return Err(account_changed());
    }
    state.session_info(&headers, true)?;
    Ok(Json(
        json!({"state":"signed_out","remembered_session_removed":!state.inner.remember_enabled || (vault.error.is_none() && !vault.stored),"vault":vault,"remote_revoked":outcome.remote_revoked}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationRequest {
    operation_id: String,
    variables: Value,
}

async fn validate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OperationRequest>,
) -> Result<Json<Value>, ApiError> {
    // Authentication must still hold after potentially slow body extraction.
    state.session_info(&headers, true)?;
    let result = if state.inner.demo {
        rivian_core::validate_request(&body.operation_id, &body.variables)
    } else {
        rivian_core::live::validate_request(&body.operation_id, &body.variables)
    };
    Ok(Json(json!(result)))
}

async fn execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Extension(epoch): axum::Extension<NativeEpoch>,
    Json(body): Json<OperationRequest>,
) -> Result<Json<Value>, ApiError> {
    state.session_info(&headers, true)?;
    if !state.inner.demo {
        if !rivian_core::live::validate_request(&body.operation_id, &body.variables).valid {
            return Err(ApiError::invalid());
        }
        if rivian_core::live::requires_confirmation(&body.operation_id) {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "confirmation_required",
                message: "Review and confirm this change before it is sent.",
            });
        }
        let client = state.live_client(&headers, true)?;
        let data = client
            .execute_read_at_epoch(&body.operation_id, &body.variables, account_epoch(epoch)?)
            .await
            .map_err(upstream_error)?;
        state.session_info(&headers, true)?;
        return Ok(Json(json!({"data": data, "mode": "live"})));
    }
    if !rivian_core::validate_request(&body.operation_id, &body.variables).valid {
        return Err(ApiError::invalid());
    }
    let data = rivian_core::execute_demo(&body.operation_id, &body.variables)
        .map_err(|_| ApiError::invalid())?;
    Ok(Json(json!({"data": data, "mode": "demo"})))
}

#[derive(Clone, Copy)]
struct NativeEpoch(Option<u64>);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyRequest {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RememberRequest {
    remember: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TelemetryRequest {
    vehicle_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentRequest {
    endpoint: String,
    operation_name: String,
    document: String,
    variables: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfirmationRequest {
    confirmation_id: String,
}

fn account_epoch(epoch: NativeEpoch) -> Result<u64, ApiError> {
    epoch.0.ok_or_else(ApiError::unauthorized)
}
fn account_changed() -> ApiError {
    ApiError {
        status: StatusCode::UNAUTHORIZED,
        code: "account_auth_required",
        message: "The account connection changed. Reconnect before trying again.",
    }
}

async fn account_vault(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, false)?;
    let result = client.vault_status().await;
    state.session_info(&headers, false)?;
    Ok(Json(json!(result)))
}
async fn account_remember(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Extension(epoch): axum::Extension<NativeEpoch>,
    Json(body): Json<RememberRequest>,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, true)?;
    let result = client
        .remember_at_epoch(body.remember, account_epoch(epoch)?)
        .await
        .map_err(upstream_error)?;
    state.session_info(&headers, true)?;
    Ok(Json(json!(result)))
}
async fn account_resend(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Extension(epoch): axum::Extension<NativeEpoch>,
    Json(_body): Json<EmptyRequest>,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, true)?;
    let result = client
        .resend_otp_at_epoch(account_epoch(epoch)?)
        .await
        .map_err(upstream_error)?;
    state.session_info(&headers, true)?;
    Ok(Json(json!(result)))
}
async fn telemetry_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Extension(epoch): axum::Extension<NativeEpoch>,
    Json(body): Json<TelemetryRequest>,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, true)?;
    let credentials = client
        .subscription_credentials_at_epoch(&body.vehicle_id, account_epoch(epoch)?)
        .await
        .map_err(upstream_error)?;
    let service = state.telemetry(&headers, true)?;
    service.start(credentials).await.map_err(upstream_error)?;
    state.session_info(&headers, true)?;
    Ok(Json(json!(service.snapshot().await)))
}
async fn telemetry_stop(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Extension(epoch): axum::Extension<NativeEpoch>,
    Json(_body): Json<EmptyRequest>,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, true)?;
    if client.authorization_epoch() != account_epoch(epoch)? {
        return Err(account_changed());
    }
    let service = state.telemetry(&headers, true)?;
    service
        .stop_at_epoch(account_epoch(epoch)?)
        .await
        .map_err(upstream_error)?;
    state.session_info(&headers, true)?;
    Ok(Json(json!(service.snapshot().await)))
}
async fn telemetry_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, false)?;
    let service = state.telemetry(&headers, false)?;
    let snapshot = service.snapshot().await;
    if let Some(source_epoch) = snapshot.authorization_epoch {
        if client.authorization_epoch() != source_epoch {
            // A delayed poll must neither return an earlier account's readings
            // nor stop or invalidate a newer connection.
            state.session_info(&headers, false)?;
            return Ok(Json(
                json!({"state":"stopped","vehicle_id":null,"compatibility":"unverified","readings":null,"raw_topics":[],"error_code":"session_revoked"}),
            ));
        }
        if snapshot.state == rivian_api::telemetry::ConnectionState::AuthenticationRequired
            && snapshot.error_code == Some("account_auth_required")
        {
            client.require_sign_in_at_epoch(source_epoch).await;
        }
    }
    state.session_info(&headers, false)?;
    Ok(Json(json!(snapshot)))
}
async fn admit_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DocumentRequest>,
) -> Result<Json<Value>, ApiError> {
    state.live_client(&headers, true)?;
    let operation_id =
        rivian_core::live::admit_document(&body.endpoint, &body.operation_name, &body.document)
            .ok_or_else(ApiError::invalid)?;
    if !rivian_core::live::validate_request(&operation_id, &body.variables).valid {
        return Err(ApiError::invalid());
    }
    Ok(Json(
        json!({"admitted":true,"requires_confirmation":rivian_core::live::requires_confirmation(&operation_id),"operation_id":operation_id,"variables":body.variables}),
    ))
}
async fn prepare_command(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Extension(epoch): axum::Extension<NativeEpoch>,
    Json(body): Json<OperationRequest>,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, true)?;
    let epoch = account_epoch(epoch)?;
    if !rivian_core::live::requires_confirmation(&body.operation_id)
        || !rivian_core::live::validate_request(&body.operation_id, &body.variables).valid
    {
        return Err(ApiError::invalid());
    }
    let vehicle = body
        .variables
        .get("vehicle_id")
        .and_then(Value::as_str)
        .ok_or_else(ApiError::invalid)?;
    client
        .check_vehicle_access_at_epoch(vehicle, epoch)
        .await
        .map_err(upstream_error)?;
    state.session_info(&headers, true)?;
    let id = random_secret().map_err(|_| ApiError::internal())?;
    let mut auth = state.inner.auth.lock().map_err(|_| ApiError::internal())?;
    let session = auth.session.as_mut().ok_or_else(ApiError::unauthorized)?;
    if !session.confirmations.issue(
        id.clone(),
        body.operation_id.clone(),
        body.variables.clone(),
        epoch,
        Instant::now(),
    ) {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "confirmation_limit",
            message: "Too many pending changes. Wait two minutes before preparing another change.",
        });
    }
    Ok(Json(
        json!({"confirmation_id":id,"operation_id":body.operation_id,"variables":body.variables,"expires_in_seconds":120}),
    ))
}
async fn execute_command(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Extension(epoch): axum::Extension<NativeEpoch>,
    Json(body): Json<ConfirmationRequest>,
) -> Result<Json<Value>, ApiError> {
    let client = state.live_client(&headers, true)?;
    let epoch = account_epoch(epoch)?;
    if body.confirmation_id.len() != 64 {
        return Err(ApiError::invalid());
    }
    let ticket = {
        let mut auth = state.inner.auth.lock().map_err(|_| ApiError::internal())?;
        auth.session.as_mut().ok_or_else(ApiError::unauthorized)?.confirmations.consume(&body.confirmation_id, epoch, Instant::now()).ok_or(ApiError { status: StatusCode::BAD_REQUEST, code: "confirmation_expired", message: "Review the change again. Its confirmation expired, was already used, or belongs to an earlier account connection." })?
    };
    let data = client.execute_confirmed(&ticket.operation_id, &ticket.variables, ticket.epoch).await.map_err(|error| {
        if matches!(error.code(), "command_outcome_unknown" | "upstream_unavailable" | "upstream_error" | "upstream_schema" | "response_too_large") {
            ApiError { status: StatusCode::BAD_GATEWAY, code: "command_outcome_unknown", message: "The change's outcome could not be confirmed. Check the vehicle name before preparing another change; it will not be retried automatically." }
        } else { upstream_error(error) }
    })?;
    state.session_info(&headers, true)?;
    Ok(Json(json!({"mode":"live","data":data})))
}

async fn static_asset(request: Request) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let path = request.uri().path().trim_start_matches('/');
    if path.starts_with("api/") || path == "api" {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error":{"code":"not_found","message":"Unknown endpoint."}})),
        )
            .into_response();
    }
    let path = if path.is_empty() { "index.html" } else { path };
    // rust-embed can read its source directory in debug builds. Never permit traversal
    // or encoded path aliases even when the release build embeds files in the binary.
    if path.contains(['\\', '%'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.starts_with('.'))
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(asset) = Assets::get(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = mime_guess::from_path(path).first_or_octet_stream();
    let body = if request.method() == Method::HEAD {
        Body::empty()
    } else {
        Body::from(asset.data.into_owned())
    };
    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type.as_ref())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    response
}
