#![forbid(unsafe_code)]

//! Loopback-only application host. This milestone has no Rivian network adapter.

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
    created_at: Instant,
    last_used: Instant,
}

impl AppState {
    /// Returns a single-use launcher capability. Never log it in ordinary operation.
    pub fn new(port: u16) -> std::io::Result<(Self, String)> {
        Self::with_lifetimes(
            port,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(8 * 3600),
        )
    }

    pub fn with_lifetimes(
        port: u16,
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
        if !secret_matches(&session.secret, provided) {
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
            "mode": "demo",
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
            message: "The operation or its variables are invalid, unavailable or blocked in demo mode.",
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

async fn boundary(State(state): State<AppState>, request: Request, next: Next) -> Response {
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
    next.run(request).await
}

async fn health() -> Json<Value> {
    Json(json!({"status": "ok", "mode": "demo", "version": env!("CARGO_PKG_VERSION")}))
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
    let mut auth = state.inner.auth.lock().map_err(|_| ApiError::internal())?;
    let bootstrap = auth.bootstrap.as_ref().ok_or_else(ApiError::unauthorized)?;
    if Instant::now() >= bootstrap.expires_at {
        auth.bootstrap = None;
        return Err(ApiError::unauthorized());
    }
    if !secret_matches(&bootstrap.secret, &body.token) {
        return Err(ApiError::unauthorized());
    }
    let now = Instant::now();
    let session = Session {
        secret: random_secret().map_err(|_| ApiError::internal())?,
        csrf: random_secret().map_err(|_| ApiError::internal())?,
        created_at: now,
        last_used: now,
    };
    let cookie = format!(
        "{}={}; HttpOnly; SameSite=Strict; Path=/",
        state.inner.cookie_name, session.secret
    );
    let body = json!({"csrf_token": session.csrf, "mode": "demo", "expires_in_seconds": state.inner.session_lifetime.as_secs()});
    // The same mutex protects both consumption and session creation, including races.
    auth.bootstrap = None;
    auth.session = Some(session);
    let mut response = Json(body).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| ApiError::internal())?,
    );
    Ok(response)
}

async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(state.session_info(&headers, false)?))
}

async fn logout(State(state): State<AppState>) -> Result<Response, ApiError> {
    state
        .inner
        .auth
        .lock()
        .map_err(|_| ApiError::internal())?
        .session = None;
    let mut response = StatusCode::NO_CONTENT.into_response();
    let cookie = format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
        state.inner.cookie_name
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| ApiError::internal())?,
    );
    Ok(response)
}

async fn catalog() -> Json<Value> {
    Json(json!(rivian_core::catalog()))
}
async fn vehicles() -> Json<Value> {
    Json(json!(rivian_core::demo_vehicles()))
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
    Ok(Json(json!(rivian_core::validate_request(
        &body.operation_id,
        &body.variables
    ))))
}

async fn execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OperationRequest>,
) -> Result<Json<Value>, ApiError> {
    state.session_info(&headers, true)?;
    if !rivian_core::validate_request(&body.operation_id, &body.variables).valid {
        return Err(ApiError::invalid());
    }
    let data = rivian_core::execute_demo(&body.operation_id, &body.variables)
        .map_err(|_| ApiError::invalid())?;
    Ok(Json(json!({"data": data, "mode": "demo"})))
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
