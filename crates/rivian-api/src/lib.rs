//! Native Rivian transport. Never expose this crate's tokens to WASM.
//!
//! Protocol references (reviewed 2026-10-09):
//! https://rivian-api.kaedenb.org/app/authentication/
//! https://rivian-api.kaedenb.org/app/account/logout/
//! https://github.com/bretterer/rivian-python-client/tree/4d15dd88e74cf1a0be0bd23f46565fe89b48af44
//! No documented refresh-token exchange is assumed. Only CSRF/application-session
//! rotation is automatic; rejected user sessions require a fresh owner sign-in.

#![forbid(unsafe_code)]

mod vault;
pub use vault::VaultStatus;
pub mod telemetry;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, HeaderValue};
use rivian_core::live;
use serde::Serialize;
use serde_json::{Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::{Mutex, watch};
use zeroize::{Zeroize, Zeroizing};

const GATEWAY: &str = "https://rivian.com/api/gql/gateway/graphql";
const MAX_RESPONSE_BYTES: usize = 1_048_576;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const ROTATE_AFTER: Duration = Duration::from_secs(3600);
const MFA_LIFETIME: Duration = Duration::from_secs(600);
const MFA_RESEND_COOLDOWN: Duration = Duration::from_secs(30);
// Local retention ceiling, not an assertion of Rivian's token lifetime. Every
// restore revalidates the current account against the upstream service.
const REMEMBERED_LIFETIME: i64 = 30 * 24 * 60 * 60;
const AUTH_WINDOW: Duration = Duration::from_secs(900);
const AUTH_COOLDOWN: Duration = Duration::from_secs(3);
const CREATE_CSRF: &str =
    "mutation CreateCSRFToken { createCsrfToken { __typename csrfToken appSessionToken } }";
const LOGIN: &str = "mutation Login($email: String!, $password: String!) { login(email: $email, password: $password) { __typename ... on MobileLoginResponse { userSessionToken } ... on MobileMFALoginResponse { otpToken targetChannel { __typename } } } }";
const LOGIN_OTP: &str = "mutation LoginWithOTP($email: String!, $otpCode: String!, $otpToken: String!) { loginWithOTPV2(email: $email, otpCode: $otpCode, otpToken: $otpToken) { __typename ... on MobileLoginResponse { userSessionToken } } }";
// Pinned gateway.graphql, rivian-python-client 4d15dd88e74cf1a0be0bd23f46565fe89b48af44.
// Channel switching and OTP-token rotation are absent from this reviewed schema.
const RESEND_OTP: &str =
    "mutation ResendOTPCode($otpToken: String!) { resendOTPCode(otpToken: $otpToken) { success } }";
const LOGOUT: &str = "mutation Logout { logout { success } }";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    SignedOut,
    MfaRequired,
    Authenticated,
}

/// Deliberately contains no email, token, identifier, password, or upstream error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthStatus {
    pub state: AuthState,
    pub channel: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ApiError {
    code: &'static str,
    message: &'static str,
}

impl ApiError {
    pub fn code(&self) -> &'static str {
        self.code
    }
    pub fn message(&self) -> &'static str {
        self.message
    }
    fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }
    fn unauthenticated() -> Self {
        Self::new(
            "account_auth_required",
            "Sign in to your Rivian account again.",
        )
    }
    fn schema() -> Self {
        Self::new(
            "upstream_schema",
            "Rivian returned an unsupported response. No missing data has been invented.",
        )
    }
    fn network() -> Self {
        Self::new(
            "upstream_unavailable",
            "Rivian could not be reached. Check your connection and try again.",
        )
    }
    fn rate_limited() -> Self {
        Self::new(
            "rate_limited",
            "Requests are paused. Wait a few minutes before trying again.",
        )
    }
}

struct PendingMfa {
    email: Zeroizing<String>,
    otp_token: Zeroizing<String>,
    channel: &'static str,
    created: Instant,
    attempts: usize,
    resends: usize,
    last_resend: Option<Instant>,
    active: Arc<AtomicBool>,
}
impl Drop for PendingMfa {
    fn drop(&mut self) {
        self.active.store(false, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct State {
    epoch: u64,
    csrf: Option<Zeroizing<String>>,
    app_session: Option<Zeroizing<String>>,
    user_session: Option<Zeroizing<String>>,
    pending: Option<PendingMfa>,
    rotated: Option<Instant>,
    user_id: Option<String>,
    vehicle_ids: Vec<String>,
    account_vehicles: Vec<Value>,
    blocked_until: Option<Instant>,
    auth_attempts: VecDeque<Instant>,
    last_auth_attempt: Option<Instant>,
    remember: bool,
    remembered_at: Option<i64>,
    restore_blocked: bool,
}

impl State {
    fn clear_auth(&mut self) {
        self.csrf = None;
        self.app_session = None;
        self.user_session = None;
        self.pending = None;
        self.rotated = None;
        self.user_id = None;
        self.vehicle_ids.clear();
        self.account_vehicles.clear();
        self.remembered_at = None;
    }

    fn status(&mut self) -> AuthStatus {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.created.elapsed() >= MFA_LIFETIME)
        {
            self.clear_auth();
        }
        AuthStatus {
            state: if self.user_session.is_some() {
                AuthState::Authenticated
            } else if self.pending.is_some() {
                AuthState::MfaRequired
            } else {
                AuthState::SignedOut
            },
            channel: self.pending.as_ref().map(|p| p.channel),
        }
    }

    fn check_cooldown(&self) -> Result<(), ApiError> {
        if self
            .blocked_until
            .is_some_and(|until| until > Instant::now())
        {
            Err(ApiError::rate_limited())
        } else {
            Ok(())
        }
    }

    fn auth_attempt(&mut self, is_login: bool) -> Result<(), ApiError> {
        self.check_cooldown()?;
        let now = Instant::now();
        if self
            .last_auth_attempt
            .is_some_and(|last| now.duration_since(last) < AUTH_COOLDOWN)
        {
            return Err(ApiError::rate_limited());
        }
        while self
            .auth_attempts
            .front()
            .is_some_and(|last| now.duration_since(*last) >= AUTH_WINDOW)
        {
            self.auth_attempts.pop_front();
        }
        if is_login {
            if self.auth_attempts.len() >= 5 {
                return Err(ApiError::rate_limited());
            }
            self.auth_attempts.push_back(now);
        }
        self.last_auth_attempt = Some(now);
        Ok(())
    }

    fn observe_error(&mut self, error: &ApiError) {
        if error.code == "rate_limited" || error.code == "account_locked" {
            self.blocked_until = Some(Instant::now() + Duration::from_secs(300));
        }
    }
}

/// Native owner session. Cloning or serializing this type is intentionally absent.
/// A mutex serializes requests, refresh, and logout so auth cannot be resurrected
/// by a completed request after logout returns.
pub struct LiveClient {
    http: reqwest::Client,
    state: Arc<Mutex<State>>,
    epoch: watch::Sender<u64>,
    revoked: AtomicBool,
    vault: vault::Vault,
    // Test-only endpoint override is not built into production or reachable from
    // any owner/developer request, environment variable, or command-line flag.
    #[cfg(test)]
    test_endpoint: Option<String>,
}
impl Drop for LiveClient {
    fn drop(&mut self) {
        self.revoke();
    }
}

/// Native-only authority snapshot. The host can pass it to telemetry but cannot
/// manufacture one or serialize its session headers into browser responses.
pub struct SubscriptionCredentials {
    pub(crate) vehicle_id: String,
    pub(crate) csrf: Zeroizing<String>,
    pub(crate) app_session: Zeroizing<String>,
    pub(crate) user_session: Zeroizing<String>,
    pub(crate) epoch: u64,
    pub(crate) revocation: watch::Receiver<u64>,
}
impl SubscriptionCredentials {
    pub fn vehicle_id(&self) -> &str {
        &self.vehicle_id
    }
    pub(crate) fn is_current(&self) -> bool {
        *self.revocation.borrow() == self.epoch && self.revocation.has_changed().is_ok()
    }
}

impl LiveClient {
    pub fn new() -> Result<Self, ApiError> {
        Self::with_vault(vault::Vault::native())
    }

    fn with_vault(vault: vault::Vault) -> Result<Self, ApiError> {
        let http = Self::http_builder()
            .https_only(true)
            .build()
            .map_err(|_| ApiError::network())?;
        Ok(Self {
            http,
            state: Arc::new(Mutex::new(State::default())),
            epoch: watch::channel(0).0,
            revoked: AtomicBool::new(false),
            vault,
            #[cfg(test)]
            test_endpoint: None,
        })
    }

    /// A native memory-only session that cannot access any operating-system
    /// credential entry. Used by host boundary tests and embedding environments
    /// that explicitly do not support remembering owner sessions.
    pub fn memory_only() -> Result<Self, ApiError> {
        Self::with_vault(vault::Vault::unavailable())
    }

    pub fn authorization_epoch(&self) -> u64 {
        *self.epoch.borrow()
    }

    fn advance_epoch(&self) -> u64 {
        let mut epoch = self.authorization_epoch();
        self.epoch.send_modify(|current| {
            *current = current.wrapping_add(1);
            epoch = *current;
        });
        epoch
    }

    fn advance_epoch_if_current(&self, expected_epoch: u64) -> Option<u64> {
        let mut epoch = expected_epoch;
        self.epoch
            .send_if_modified(|current| {
                if *current != expected_epoch {
                    return false;
                }
                *current = current.wrapping_add(1);
                epoch = *current;
                true
            })
            .then_some(epoch)
    }

    /// A transport has rejected the owner session. Cancel other authority
    /// snapshots and forget the expired native bundle without replaying login.
    pub async fn require_sign_in(&self) -> AuthStatus {
        self.require_sign_in_at_epoch(self.authorization_epoch())
            .await
    }

    pub async fn require_sign_in_at_epoch(&self, expected_epoch: u64) -> AuthStatus {
        let Some(invalidation_epoch) = self.advance_epoch_if_current(expected_epoch) else {
            return self.status().await;
        };
        self.finish_account_invalidation(invalidation_epoch).await
    }

    async fn finish_account_invalidation(&self, invalidation_epoch: u64) -> AuthStatus {
        let mut state = self.state.lock().await;
        if self.authorization_epoch() != invalidation_epoch {
            return self.scoped_status(&mut state);
        }
        state.epoch = invalidation_epoch;
        state.clear_auth();
        state.remember = false;
        state.restore_blocked = true;
        let _ = self.vault.delete().await;
        state.status()
    }

    fn schedule_mfa_expiry(&self, epoch: u64, created: Instant, active: Arc<AtomicBool>) {
        let state = Arc::downgrade(&self.state);
        let mut canceled = self.epoch.subscribe();
        let expiration = self.epoch.clone();
        let deadline = tokio::time::Instant::from_std(created + MFA_LIFETIME);
        // This task holds only a weak state reference and a non-secret activity
        // flag. Client::drop revokes its epoch so the task ends immediately.
        tokio::spawn(async move {
            tokio::select! {
                biased;
                _ = canceled.changed() => {}
                _ = tokio::time::sleep_until(deadline) => {
                    let expired = active.load(Ordering::SeqCst) && expiration.send_if_modified(|current| {
                        if *current == epoch { *current = current.wrapping_add(1); true } else { false }
                    });
                    // Revoke first to cancel an OTP request holding the state
                    // mutex; awaiting the mutex must not extend the challenge.
                    if expired && let Some(state) = state.upgrade() {
                        let mut state = state.lock().await;
                        if state.pending.as_ref().is_some_and(|pending| pending.created == created) {
                            state.clear_auth();
                            state.epoch = *expiration.borrow();
                        }
                    }
                }
            }
        });
    }

    pub async fn vault_status(&self) -> VaultStatus {
        let state = self.state.lock().await;
        self.vault.status(state.remember)
    }

    /// Owner opt-in controls persistence; an unavailable vault never falls back
    /// to a file. A caller may still perform a memory-only sign-in after failure.
    pub async fn remember(&self, enabled: bool) -> Result<VaultStatus, ApiError> {
        let epoch = self.authorization_epoch();
        self.remember_at_epoch(enabled, epoch).await
    }

    pub async fn remember_at_epoch(
        &self,
        enabled: bool,
        epoch: u64,
    ) -> Result<VaultStatus, ApiError> {
        let mut state = self.state.lock().await;
        self.ensure_active(epoch)?;
        self.ensure_auth_scope(&state, epoch)?;
        state.epoch = epoch;
        if !enabled {
            state.remember = false;
            self.vault.delete().await?;
        } else {
            // Read is a non-mutating availability check and does not load any
            // account authority. Only restore_remembered may validate a bundle.
            self.vault.read(self.epoch.clone(), epoch).await?;
            self.ensure_active(epoch)?;
            state.remember = true;
            if state.user_session.is_some() {
                state.remembered_at = Some(OffsetDateTime::now_utc().unix_timestamp());
                if let Err(error) = self.save_remembered(&state).await {
                    state.remember = false;
                    return Err(error);
                }
            }
        }
        self.ensure_active(epoch)?;
        Ok(self.vault.status(state.remember))
    }

    /// Load only the fixed native entry, rotate the application session, and
    /// validate account identity and current owned vehicles before authenticating.
    pub async fn restore_remembered(&self) -> Result<AuthStatus, ApiError> {
        let epoch = self.authorization_epoch();
        let mut state = self.state.lock().await;
        self.ensure_active(epoch)?;
        state.epoch = epoch;
        if state.user_session.is_some() || state.pending.is_some() {
            return Ok(state.status());
        }
        if state.restore_blocked {
            return Ok(state.status());
        }
        let Some(bytes) = self.vault.read(self.epoch.clone(), epoch).await? else {
            return Ok(state.status());
        };
        let bundle = match remembered_bundle(&bytes) {
            Ok(bundle) => bundle,
            Err(error) => {
                let _ = self.vault.delete().await;
                return Err(error);
            }
        };
        self.ensure_active(epoch)?;
        state.clear_auth();
        state.csrf = Some(bundle.csrf);
        state.app_session = Some(bundle.app_session);
        state.user_session = Some(bundle.user_session);
        state.user_id = Some(bundle.user_id);
        let result = async {
            self.rotate_csrf(&mut state).await?;
            self.discover_account(&mut state).await?;
            self.ensure_active(epoch)?;
            Ok::<(), ApiError>(())
        }
        .await;
        if let Err(error) = result {
            state.clear_auth();
            state.observe_error(&error);
            if matches!(error.code, "account_auth_required" | "upstream_schema") {
                let _ = self.vault.delete().await;
            }
            return Err(error);
        }
        state.remember = true;
        state.remembered_at = Some(bundle.saved_at);
        if let Err(error) = self.save_remembered(&state).await {
            state.remember = false;
            if error.code == "session_revoked" {
                state.clear_auth();
            }
            return Err(error);
        }
        Ok(state.status())
    }

    async fn save_remembered(&self, state: &State) -> Result<(), ApiError> {
        if !state.remember || state.user_session.is_none() {
            return Ok(());
        }
        self.ensure_active(state.epoch)?;
        let bundle = SecretJson(json!({
            "version":1,
            "saved_at":state.remembered_at.ok_or_else(ApiError::schema)?,
            "csrf":state.csrf.as_deref().ok_or_else(ApiError::schema)?.as_str(),
            "app_session":state.app_session.as_deref().ok_or_else(ApiError::schema)?.as_str(),
            "user_session":state.user_session.as_deref().ok_or_else(ApiError::schema)?.as_str(),
            "user_id":state.user_id.as_deref().ok_or_else(ApiError::schema)?
        }));
        let bytes = Zeroizing::new(serde_json::to_vec(&bundle.0).map_err(|_| ApiError::schema())?);
        self.vault
            .write(bytes, self.epoch.clone(), state.epoch)
            .await
    }

    pub async fn check_vehicle_access(&self, vehicle_id: &str) -> Result<(), ApiError> {
        let epoch = self.authorization_epoch();
        self.check_vehicle_access_at_epoch(vehicle_id, epoch).await
    }

    pub async fn check_vehicle_access_at_epoch(
        &self,
        vehicle_id: &str,
        epoch: u64,
    ) -> Result<(), ApiError> {
        let state = self.state.lock().await;
        self.ensure_active(epoch)?;
        self.ensure_auth_scope(&state, epoch)?;
        Self::owned_vehicle(&state, vehicle_id)
    }

    fn owned_vehicle(state: &State, vehicle_id: &str) -> Result<(), ApiError> {
        if state.user_session.is_none() {
            return Err(ApiError::unauthenticated());
        }
        if state.vehicle_ids.iter().any(|id| id == vehicle_id) {
            Ok(())
        } else {
            Err(ApiError::new(
                "vehicle_not_owned",
                "Choose a vehicle associated with your signed-in Rivian account.",
            ))
        }
    }

    pub async fn subscription_credentials(
        &self,
        vehicle_id: &str,
    ) -> Result<SubscriptionCredentials, ApiError> {
        let epoch = self.authorization_epoch();
        self.subscription_credentials_at_epoch(vehicle_id, epoch)
            .await
    }

    pub async fn subscription_credentials_at_epoch(
        &self,
        vehicle_id: &str,
        epoch: u64,
    ) -> Result<SubscriptionCredentials, ApiError> {
        let mut state = self.state.lock().await;
        self.ensure_active(epoch)?;
        self.ensure_auth_scope(&state, epoch)?;
        state.epoch = epoch;
        state.check_cooldown()?;
        Self::owned_vehicle(&state, vehicle_id)?;
        if state
            .rotated
            .is_none_or(|rotated| rotated.elapsed() >= ROTATE_AFTER)
        {
            self.rotate_csrf(&mut state).await?;
            if let Err(error) = self.save_remembered(&state).await {
                state.remember = false;
                if error.code == "session_revoked" {
                    state.clear_auth();
                    return Err(error);
                }
            }
        }
        self.ensure_active(epoch)?;
        Ok(SubscriptionCredentials {
            vehicle_id: vehicle_id.to_owned(),
            csrf: state.csrf.as_ref().ok_or_else(ApiError::schema)?.clone(),
            app_session: state
                .app_session
                .as_ref()
                .ok_or_else(ApiError::schema)?
                .clone(),
            user_session: state
                .user_session
                .as_ref()
                .ok_or_else(ApiError::schema)?
                .clone(),
            epoch,
            revocation: self.epoch.subscribe(),
        })
    }

    fn http_builder() -> reqwest::ClientBuilder {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(REQUEST_TIMEOUT)
            .user_agent("MyRivianData/0.1.0")
    }

    /// Permanently invalidate the local capability, canceling in-flight network
    /// requests. Safe to invoke synchronously from the host's Session::drop.
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::SeqCst);
        self.epoch
            .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
    }

    fn ensure_active(&self, epoch: u64) -> Result<(), ApiError> {
        if self.revoked.load(Ordering::SeqCst) || *self.epoch.borrow() != epoch {
            Err(ApiError::new(
                "session_revoked",
                "This local session has ended. Reopen My Rivian Data.",
            ))
        } else {
            Ok(())
        }
    }

    pub async fn status(&self) -> AuthStatus {
        let mut state = self.state.lock().await;
        if self.revoked.load(Ordering::SeqCst) {
            state.clear_auth();
        }
        self.scoped_status(&mut state)
    }

    fn scoped_status(&self, state: &mut State) -> AuthStatus {
        if state.epoch != self.authorization_epoch() {
            AuthStatus {
                state: AuthState::SignedOut,
                channel: None,
            }
        } else {
            state.status()
        }
    }

    fn ensure_auth_scope(&self, state: &State, epoch: u64) -> Result<(), ApiError> {
        if state.user_session.is_some() && state.epoch != epoch {
            Err(ApiError::unauthenticated())
        } else {
            Ok(())
        }
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthStatus, ApiError> {
        self.login_at_epoch(email, password, self.authorization_epoch())
            .await
    }

    pub async fn login_with_remember(
        &self,
        email: &str,
        password: &str,
        remember: bool,
        expected_epoch: u64,
    ) -> Result<AuthStatus, ApiError> {
        validate_login_input(email, password)?;
        let preference = self.remember_at_epoch(remember, expected_epoch).await;
        // Unavailable vaults may use session memory, but a revoked admission
        // epoch never becomes a fresh post-logout login.
        self.ensure_active(expected_epoch)?;
        if let Err(error) = preference
            && error.code == "session_revoked"
        {
            return Err(error);
        }
        self.login_at_epoch(email, password, expected_epoch).await
    }

    async fn login_at_epoch(
        &self,
        email: &str,
        password: &str,
        expected_epoch: u64,
    ) -> Result<AuthStatus, ApiError> {
        let email = validate_login_input(email, password)?;
        self.ensure_active(expected_epoch)?;
        // The comparison and increment share the watch writer lock: logout
        // cannot fit between a check and a fresh generation capture.
        let epoch = self
            .advance_epoch_if_current(expected_epoch)
            .ok_or_else(|| {
                ApiError::new(
                    "session_revoked",
                    "This local session has ended. Reopen My Rivian Data.",
                )
            })?;
        let mut state = self.state.lock().await;
        self.ensure_active(epoch)?;
        state.epoch = epoch;
        state.clear_auth();
        state.restore_blocked = true;
        if self.vault.status(false).stored {
            let _ = self.vault.delete().await;
        }
        self.ensure_active(epoch)?;
        state.auth_attempt(true)?;
        let outcome = self
            .login_inner(&mut state, email, password)
            .await
            .and_then(|status| {
                self.ensure_active(epoch)?;
                Ok(status)
            });
        if let Err(error) = &outcome {
            state.clear_auth();
            state.observe_error(error);
        } else if let Some(pending) = &state.pending {
            self.schedule_mfa_expiry(epoch, pending.created, pending.active.clone());
        } else if state.user_session.is_some() {
            state.remembered_at = Some(OffsetDateTime::now_utc().unix_timestamp());
            if let Err(error) = self.save_remembered(&state).await {
                state.remember = false;
                if error.code == "session_revoked" {
                    state.clear_auth();
                    return Err(error);
                }
            }
        }
        if let Err(error) = self.ensure_active(epoch) {
            state.clear_auth();
            return Err(error);
        }
        outcome
    }

    async fn login_inner(
        &self,
        state: &mut State,
        email: &str,
        password: &str,
    ) -> Result<AuthStatus, ApiError> {
        self.rotate_csrf(state).await?;
        let response = SecretJson(
            self.post(
                GATEWAY,
                "Login",
                LOGIN,
                json!({"email":email,"password":password}),
                state,
            )
            .await?,
        );
        let login = response
            .0
            .pointer("/data/login")
            .ok_or_else(ApiError::schema)?;
        if login.get("__typename").and_then(Value::as_str) == Some("MobileMFALoginResponse") {
            state.pending = Some(PendingMfa {
                email: Zeroizing::new(email.to_owned()),
                otp_token: secret_field(login, "otpToken")?,
                channel: match login
                    .pointer("/targetChannel/__typename")
                    .and_then(Value::as_str)
                {
                    Some("MfaEmailChannel") => "email",
                    Some("MfaPhoneChannel") => "text message",
                    Some("MfaAuthenticatorChannel") => "authenticator",
                    _ => "authenticator or your selected Rivian verification method",
                },
                created: Instant::now(),
                attempts: 0,
                resends: 0,
                last_resend: None,
                active: Arc::new(AtomicBool::new(true)),
            });
        } else {
            state.user_session = Some(secret_field(login, "userSessionToken")?);
            self.discover_account(state).await?;
        }
        Ok(state.status())
    }

    pub async fn verify_otp(&self, code: &str) -> Result<AuthStatus, ApiError> {
        self.verify_otp_at_epoch(code, self.authorization_epoch())
            .await
    }

    pub async fn verify_otp_at_epoch(
        &self,
        code: &str,
        epoch: u64,
    ) -> Result<AuthStatus, ApiError> {
        if !(6..=8).contains(&code.len()) || !code.bytes().all(|c| c.is_ascii_digit()) {
            return Err(ApiError::new(
                "invalid_input",
                "Enter the verification code from Rivian or your authenticator.",
            ));
        }
        let mut state = self.state.lock().await;
        self.ensure_active(epoch)?;
        state.epoch = epoch;
        state.auth_attempt(false)?;
        state.status();
        let pending = state
            .pending
            .as_mut()
            .ok_or_else(|| ApiError::new("mfa_expired", "Verification expired. Sign in again."))?;
        if pending.attempts >= 5 {
            state.clear_auth();
            return Err(ApiError::new(
                "mfa_expired",
                "Too many verification attempts. Sign in again after waiting.",
            ));
        }
        pending.attempts += 1;
        let variables = json!({"email":pending.email.as_str(),"otpToken":pending.otp_token.as_str(),"otpCode":code});
        let response = match self
            .post(GATEWAY, "LoginWithOTP", LOGIN_OTP, variables, &state)
            .await
        {
            Ok(response) => SecretJson(response),
            Err(error) => {
                state.observe_error(&error);
                if !matches!(
                    error.code,
                    "invalid_credentials" | "invalid_otp" | "rate_limited" | "upstream_unavailable"
                ) || state
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.attempts >= 5)
                {
                    state.clear_auth();
                }
                return Err(if error.code == "invalid_credentials" {
                    ApiError::new(
                        "invalid_otp",
                        "The verification code was not accepted. Check the code and try again.",
                    )
                } else {
                    error
                });
            }
        };
        let token = response
            .0
            .pointer("/data/loginWithOTPV2")
            .ok_or_else(ApiError::schema)
            .and_then(|login| secret_field(login, "userSessionToken"));
        match token {
            Ok(token) => state.user_session = Some(token),
            Err(error) => {
                state.clear_auth();
                return Err(error);
            }
        }
        state.pending = None;
        if let Err(error) = self.discover_account(&mut state).await {
            state.clear_auth();
            state.observe_error(&error);
            return Err(error);
        }
        self.ensure_active(epoch)?;
        state.remembered_at = Some(OffsetDateTime::now_utc().unix_timestamp());
        if let Err(error) = self.save_remembered(&state).await {
            state.remember = false;
            if error.code == "session_revoked" {
                state.clear_auth();
                return Err(error);
            }
        }
        self.ensure_active(epoch)?;
        Ok(state.status())
    }

    /// The reviewed resend mutation only targets the current pending challenge.
    /// Its secret and any upstream diagnostics stay native.
    pub async fn resend_otp(&self) -> Result<AuthStatus, ApiError> {
        let epoch = self.authorization_epoch();
        self.resend_otp_at_epoch(epoch).await
    }

    pub async fn resend_otp_at_epoch(&self, epoch: u64) -> Result<AuthStatus, ApiError> {
        let mut state = self.state.lock().await;
        self.ensure_active(epoch)?;
        state.epoch = epoch;
        state.status();
        let pending = state
            .pending
            .as_ref()
            .ok_or_else(|| ApiError::new("mfa_expired", "Verification expired. Sign in again."))?;
        if pending.resends >= 3 {
            state.clear_auth();
            return Err(ApiError::new(
                "mfa_expired",
                "The resend limit was reached. Sign in again after waiting.",
            ));
        }
        if pending
            .last_resend
            .is_some_and(|last| last.elapsed() < MFA_RESEND_COOLDOWN)
        {
            return Err(ApiError::rate_limited());
        }
        state.auth_attempt(false)?;
        let pending = state.pending.as_mut().ok_or_else(ApiError::schema)?;
        pending.resends += 1;
        pending.last_resend = Some(Instant::now());
        let variables = json!({"otpToken":pending.otp_token.as_str()});
        let response = match self
            .post(GATEWAY, "ResendOTPCode", RESEND_OTP, variables, &state)
            .await
        {
            Ok(response) => SecretJson(response),
            Err(error) => {
                state.observe_error(&error);
                if !matches!(error.code, "rate_limited" | "upstream_unavailable") {
                    state.clear_auth();
                }
                return Err(error);
            }
        };
        if response
            .0
            .pointer("/data/resendOTPCode/success")
            .and_then(Value::as_bool)
            != Some(true)
        {
            state.clear_auth();
            return Err(ApiError::schema());
        }
        self.ensure_active(epoch)?;
        Ok(state.status())
    }

    /// Best-effort upstream revocation; local secrets are erased even if Rivian
    /// is unreachable. The boolean says whether Rivian confirmed revocation.
    pub async fn logout(&self) -> bool {
        // Increment before waiting for the mutex: this cancels older requests
        // and ensures a queued old login cannot resurrect the signed-out account.
        let epoch = self.advance_epoch();
        self.finish_logout(epoch).await
    }

    async fn finish_logout(&self, epoch: u64) -> bool {
        let mut state = self.state.lock().await;
        if self.authorization_epoch() != epoch && !self.revoked.load(Ordering::SeqCst) {
            return false;
        }
        state.epoch = epoch;
        let confirmed = if state.user_session.is_none() {
            true
        } else if self.ensure_active(epoch).is_ok() {
            self.post(GATEWAY, "Logout", LOGOUT, json!({}), &state)
                .await
                .ok()
                .and_then(|v| v.pointer("/data/logout/success").and_then(Value::as_bool))
                .unwrap_or(false)
        } else {
            false
        };
        if self.authorization_epoch() != epoch && !self.revoked.load(Ordering::SeqCst) {
            return false;
        }
        state.clear_auth();
        state.remember = false;
        state.restore_blocked = true;
        // Even a permanently revoked client must delete its remembered secret.
        // Failure is retained in vault_status, independent of upstream success.
        let _ = self.vault.delete().await;
        confirmed
    }

    pub async fn vehicles(&self) -> Result<Value, ApiError> {
        self.vehicles_at_epoch(self.authorization_epoch()).await
    }

    pub async fn vehicles_at_epoch(&self, epoch: u64) -> Result<Value, ApiError> {
        let response = self
            .execute_read_at_epoch("list-vehicles", &json!({}), epoch)
            .await?;
        Ok(response["data"]["vehicles"].clone())
    }

    /// Return only allowlisted data-operation responses. Authentication methods
    /// never return their raw responses, even in the developer explorer.
    pub async fn execute(&self, operation_id: &str, variables: &Value) -> Result<Value, ApiError> {
        self.execute_read_at_epoch(operation_id, variables, self.authorization_epoch())
            .await
    }

    pub async fn execute_read_at_epoch(
        &self,
        operation_id: &str,
        variables: &Value,
        expected_epoch: u64,
    ) -> Result<Value, ApiError> {
        if live::requires_confirmation(operation_id) {
            return Err(ApiError::new(
                "confirmation_required",
                "Confirm this change before it is sent to Rivian.",
            ));
        }
        self.execute_at_epoch(operation_id, variables, expected_epoch)
            .await
    }

    pub async fn execute_confirmed(
        &self,
        operation_id: &str,
        variables: &Value,
        expected_epoch: u64,
    ) -> Result<Value, ApiError> {
        if !live::requires_confirmation(operation_id) {
            return Err(ApiError::new(
                "invalid_operation",
                "This operation does not use a confirmation capability.",
            ));
        }
        self.execute_at_epoch(operation_id, variables, expected_epoch)
            .await
    }

    async fn execute_at_epoch(
        &self,
        operation_id: &str,
        variables: &Value,
        epoch: u64,
    ) -> Result<Value, ApiError> {
        let mut state = self.state.lock().await;
        self.ensure_active(epoch)?;
        self.ensure_auth_scope(&state, epoch)?;
        state.epoch = epoch;
        state.check_cooldown()?;
        if state.user_session.is_none() {
            return Err(ApiError::unauthenticated());
        }
        if let Some(vehicle_id) = variables.get("vehicle_id").and_then(Value::as_str)
            && !state.vehicle_ids.iter().any(|id| id == vehicle_id)
        {
            return Err(ApiError::new(
                "vehicle_not_owned",
                "Choose a vehicle associated with your signed-in Rivian account.",
            ));
        }
        let request =
            live::request(operation_id, variables, state.user_id.as_deref()).map_err(|_| {
                ApiError::new(
                    "invalid_operation",
                    "This operation or its inputs are not supported.",
                )
            })?;
        if state
            .rotated
            .is_none_or(|rotated| rotated.elapsed() >= ROTATE_AFTER)
            && let Err(error) = self.rotate_csrf(&mut state).await
        {
            state.observe_error(&error);
            if self.revoked.load(Ordering::SeqCst) {
                state.clear_auth();
            }
            return Err(error);
        }
        if let Err(error) = self
            .authorize_resource(operation_id, variables, &state)
            .await
        {
            state.observe_error(&error);
            if error.code == "account_auth_required" {
                self.forget_expired_auth(&mut state).await;
            }
            return Err(error);
        }
        let mut result = self
            .post(
                request.endpoint.url(),
                &request.operation_name,
                &request.query,
                request.variables.clone(),
                &state,
            )
            .await;
        // Read-only operations may rotate the app session once;
        // no password replay and no repeated retry loops on user-session expiry.
        if !live::requires_confirmation(operation_id)
            && result
                .as_ref()
                .is_err_and(|e| e.code == "account_auth_required")
        {
            result = match self.rotate_csrf(&mut state).await {
                Ok(()) => {
                    self.post(
                        request.endpoint.url(),
                        &request.operation_name,
                        &request.query,
                        request.variables,
                        &state,
                    )
                    .await
                }
                Err(error) => Err(error),
            };
        }
        match result {
            Ok(mut response) => {
                self.ensure_active(epoch)?;
                redact_secrets(&mut response);
                if (operation_id == "list-vehicles" || operation_id == "account-summary")
                    && let Err(error) = Self::remember_account(&mut state, &response)
                {
                    self.forget_expired_auth(&mut state).await;
                    return Err(error);
                }
                let normalized =
                    live::normalize(operation_id, variables, &response, &state.account_vehicles)
                        .map_err(|_| ApiError::schema())?;
                let observed_at = normalized
                    .get("observed_at")
                    .filter(|value| value.is_string())
                    .cloned()
                    .unwrap_or(Value::Null);
                let received_at = OffsetDateTime::now_utc()
                    .format(&Rfc3339)
                    .map_err(|_| ApiError::schema())?;
                if let Err(error) = self.save_remembered(&state).await {
                    state.remember = false;
                    if error.code == "session_revoked" {
                        state.clear_auth();
                        return Err(error);
                    }
                }
                self.ensure_active(epoch)?;
                Ok(
                    json!({"operation_id":operation_id,"source":"rivian","synthetic":false,"observed_at":observed_at,"received_at":received_at,"data":normalized,"raw_response":response}),
                )
            }
            Err(error) => {
                state.observe_error(&error);
                if error.code == "account_auth_required" || self.revoked.load(Ordering::SeqCst) {
                    state.clear_auth();
                    if error.code == "account_auth_required" {
                        self.forget_expired_auth(&mut state).await;
                    }
                }
                Err(error)
            }
        }
    }

    async fn forget_expired_auth(&self, state: &mut State) {
        self.epoch
            .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
        state.epoch = self.authorization_epoch();
        state.clear_auth();
        state.remember = false;
        state.restore_blocked = true;
        let _ = self.vault.delete().await;
    }

    async fn authorize_resource(
        &self,
        operation_id: &str,
        variables: &Value,
        state: &State,
    ) -> Result<(), ApiError> {
        let (authority_operation, path, id_field, resource_id) =
            match live::resource_id_field(operation_id) {
                Some("order_id") => (
                    "vehicle-orders",
                    "/data/user/orderSnapshots",
                    "id",
                    variables["order_id"].as_str(),
                ),
                Some("wallbox_id") => (
                    "registered-wallboxes",
                    "/data/getRegisteredWallboxes",
                    "wallboxId",
                    variables["wallbox_id"].as_str(),
                ),
                _ => return Ok(()),
            };
        let resource_id = resource_id.ok_or_else(ApiError::schema)?;
        let authority = live::request(authority_operation, &json!({}), state.user_id.as_deref())
            .map_err(|_| ApiError::schema())?;
        let response = SecretJson(
            self.post(
                authority.endpoint.url(),
                &authority.operation_name,
                &authority.query,
                authority.variables,
                state,
            )
            .await?,
        );
        let items = response
            .0
            .pointer(path)
            .and_then(Value::as_array)
            .ok_or_else(ApiError::schema)?;
        let user_id = state.user_id.as_deref().ok_or_else(ApiError::schema)?;
        let owned = items.iter().any(|item| {
            item[id_field].as_str() == Some(resource_id)
                && (authority_operation != "registered-wallboxes"
                    || item["userId"].as_str() == Some(user_id))
        });
        if owned {
            Ok(())
        } else {
            Err(ApiError::new(
                "resource_not_owned",
                "Choose an order or wall charger associated with your signed-in Rivian account.",
            ))
        }
    }

    async fn discover_account(&self, state: &mut State) -> Result<(), ApiError> {
        let request =
            live::request("list-vehicles", &json!({}), None).map_err(|_| ApiError::schema())?;
        let mut response = self
            .post(
                request.endpoint.url(),
                &request.operation_name,
                &request.query,
                request.variables,
                state,
            )
            .await?;
        redact_secrets(&mut response);
        Self::remember_account(state, &response)
    }

    fn remember_account(state: &mut State, response: &Value) -> Result<(), ApiError> {
        let identity = live::account_identity(response).map_err(|_| ApiError::schema())?;
        if state
            .user_id
            .as_ref()
            .is_some_and(|id| id != &identity.user_id)
        {
            state.clear_auth();
            return Err(ApiError::schema());
        }
        state.user_id = Some(identity.user_id);
        state.vehicle_ids = identity.vehicle_ids;
        state.account_vehicles = live::normalize("list-vehicles", &json!({}), response, &[])
            .map_err(|_| ApiError::schema())?["vehicles"]
            .as_array()
            .ok_or_else(ApiError::schema)?
            .clone();
        Ok(())
    }

    async fn rotate_csrf(&self, state: &mut State) -> Result<(), ApiError> {
        // CSRF acquisition does not forward any existing credential.
        let response = SecretJson(
            self.post(
                GATEWAY,
                "CreateCSRFToken",
                CREATE_CSRF,
                json!({}),
                &State {
                    epoch: state.epoch,
                    ..State::default()
                },
            )
            .await?,
        );
        let tokens = response
            .0
            .pointer("/data/createCsrfToken")
            .ok_or_else(ApiError::schema)?;
        let csrf = secret_field(tokens, "csrfToken")?;
        let app = secret_field(tokens, "appSessionToken")?;
        state.csrf = Some(csrf);
        state.app_session = Some(app);
        state.rotated = Some(Instant::now());
        Ok(())
    }

    async fn post(
        &self,
        endpoint: &str,
        operation: &str,
        query: &str,
        variables: Value,
        state: &State,
    ) -> Result<Value, ApiError> {
        let mut changed = self.epoch.subscribe();
        self.ensure_active(state.epoch)?;
        tokio::select! {
            biased;
            _ = changed.changed() => Err(ApiError::new("session_revoked", "This local session has ended. Reopen My Rivian Data.")),
            result = self.post_inner(endpoint, operation, query, variables, state) => {
                self.ensure_active(state.epoch)?;
                result
            }
        }
    }

    async fn post_inner(
        &self,
        endpoint: &str,
        operation: &str,
        query: &str,
        variables: Value,
        state: &State,
    ) -> Result<Value, ApiError> {
        // Defense in depth: never turn this helper into an arbitrary URL proxy.
        if !matches!(
            endpoint,
            GATEWAY
                | "https://rivian.com/api/gql/chrg/user/graphql"
                | "https://rivian.com/api/gql/orders/graphql"
                | "https://rivian.com/api/gql/t2d/graphql"
        ) {
            return Err(ApiError::new(
                "invalid_operation",
                "The requested service is not supported.",
            ));
        }
        #[cfg(test)]
        let endpoint = self.test_endpoint.as_deref().unwrap_or(endpoint);
        let mut headers = HeaderMap::new();
        headers.insert("accept", HeaderValue::from_static("application/json"));
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert(
            "apollographql-client-name",
            HeaderValue::from_static("com.rivian.android.consumer"),
        );
        for (name, secret) in [
            ("csrf-token", &state.csrf),
            ("a-sess", &state.app_session),
            ("u-sess", &state.user_session),
        ] {
            if let Some(secret) = secret {
                let mut value = HeaderValue::from_str(secret).map_err(|_| ApiError::schema())?;
                value.set_sensitive(true);
                headers.insert(name, value);
            }
        }
        let body =
            SecretJson(json!({"operationName":operation,"query":query,"variables":variables}));
        let encoded = Zeroizing::new(serde_json::to_vec(&body.0).map_err(|_| ApiError::schema())?);
        let mut response = self
            .http
            .post(endpoint)
            .headers(headers)
            .body(encoded.to_vec())
            .send()
            .await
            .map_err(|_| ApiError::network())?;
        let status = response.status();
        if status.as_u16() == 429 {
            return Err(ApiError::rate_limited());
        }
        if status.as_u16() == 401 {
            return Err(ApiError::unauthenticated());
        }
        if status.as_u16() == 403 {
            return Err(ApiError::new(
                "upstream_denied",
                "Rivian declined this request. Wait before trying again; the unofficial API may have changed.",
            ));
        }
        if !status.is_success() {
            return Err(ApiError::network());
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
        {
            return Err(ApiError::new(
                "response_too_large",
                "Rivian returned more data than this request allows.",
            ));
        }
        let mut bytes = Zeroizing::new(Vec::new());
        while let Some(chunk) = response.chunk().await.map_err(|_| ApiError::network())? {
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(ApiError::new(
                    "response_too_large",
                    "Rivian returned more data than this request allows.",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let response = serde_json::from_slice::<Value>(&bytes).map_err(|_| ApiError::schema())?;
        if let Some(error) = graphql_error(&response) {
            // Ensure sensitive upstream strings are erased without logging.
            drop(SecretJson(response));
            return Err(error);
        }
        if !response.get("data").is_some_and(Value::is_object) {
            drop(SecretJson(response));
            return Err(ApiError::schema());
        }
        Ok(response)
    }
}

fn validate_login_input<'a>(email: &'a str, password: &str) -> Result<&'a str, ApiError> {
    let email = email.trim();
    if email.is_empty()
        || email.len() > 254
        || !email.contains('@')
        || email.chars().any(char::is_control)
        || password.is_empty()
        || password.len() > 1024
    {
        Err(ApiError::new(
            "invalid_input",
            "Enter your Rivian account email and password.",
        ))
    } else {
        Ok(email)
    }
}

struct RememberedBundle {
    saved_at: i64,
    csrf: Zeroizing<String>,
    app_session: Zeroizing<String>,
    user_session: Zeroizing<String>,
    user_id: String,
}

fn remembered_bundle(bytes: &[u8]) -> Result<RememberedBundle, ApiError> {
    let invalid = || {
        ApiError::new(
            "remembered_session_invalid",
            "The remembered session expired or is invalid. Sign in again.",
        )
    };
    if bytes.len() > 32_768 {
        return Err(invalid());
    }
    let value = SecretJson(serde_json::from_slice::<Value>(bytes).map_err(|_| invalid())?);
    let fields = value.0.as_object().ok_or_else(invalid)?;
    if fields.len() != 6 || value.0["version"] != 1 {
        return Err(invalid());
    }
    let saved_at = value.0["saved_at"].as_i64().ok_or_else(invalid)?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    if saved_at > now || now.saturating_sub(saved_at) >= REMEMBERED_LIFETIME {
        return Err(invalid());
    }
    let user_id = value.0["user_id"].as_str().ok_or_else(invalid)?;
    if user_id.is_empty() || user_id.len() > 254 || user_id.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(RememberedBundle {
        saved_at,
        csrf: secret_field(&value.0, "csrf").map_err(|_| invalid())?,
        app_session: secret_field(&value.0, "app_session").map_err(|_| invalid())?,
        user_session: secret_field(&value.0, "user_session").map_err(|_| invalid())?,
        user_id: user_id.to_owned(),
    })
}

fn secret_field(value: &Value, field: &str) -> Result<Zeroizing<String>, ApiError> {
    let token = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(ApiError::schema)?;
    if token.is_empty() || token.len() > 8192 || HeaderValue::from_str(token).is_err() {
        return Err(ApiError::schema());
    }
    Ok(Zeroizing::new(token.to_owned()))
}

fn graphql_error(value: &Value) -> Option<ApiError> {
    let raw_errors = value.get("errors")?;
    if raw_errors.is_null() {
        return None;
    }
    let Some(errors) = raw_errors.as_array() else {
        return Some(ApiError::schema());
    };
    if errors.is_empty() {
        return None;
    }
    for error in errors {
        match error.pointer("/extensions/code").and_then(Value::as_str) {
            Some("UNAUTHENTICATED" | "UNAUTHORIZED") => return Some(ApiError::unauthenticated()),
            Some("RATE_LIMIT" | "RATE_LIMITED" | "TOO_MANY_REQUESTS") => {
                return Some(ApiError::rate_limited());
            }
            Some("SESSION_MANAGER_ERROR") => {
                return Some(ApiError::new(
                    "account_locked",
                    "Rivian temporarily locked this session. Wait before signing in again.",
                ));
            }
            Some("BAD_CURRENT_PASSWORD" | "INVALID_CREDENTIALS" | "BAD_CREDENTIALS") => {
                return Some(ApiError::new(
                    "invalid_credentials",
                    "The account email or password was not accepted.",
                ));
            }
            Some("INVALID_OTP" | "BAD_OTP" | "BAD_OTP_CODE") => {
                return Some(ApiError::new(
                    "invalid_otp",
                    "The verification code was not accepted. Check the code and try again.",
                ));
            }
            _ => {}
        }
    }
    Some(ApiError::new(
        "upstream_error",
        "Rivian could not complete this request. Its unofficial API or account permissions may have changed.",
    ))
}

fn redact_secrets(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let name = key.to_ascii_lowercase().replace(['_', '-'], "");
                if name.contains("token")
                    || name.contains("password")
                    || name.contains("secret")
                    || matches!(
                        name.as_str(),
                        "asess" | "usess" | "authorization" | "cookie" | "setcookie" | "pin"
                    )
                {
                    erase_json(value);
                    *value = Value::String("[redacted]".to_owned());
                } else {
                    redact_secrets(value);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact_secrets),
        _ => {}
    }
}

struct SecretJson(Value);
impl Drop for SecretJson {
    fn drop(&mut self) {
        erase_json(&mut self.0);
    }
}
fn erase_json(value: &mut Value) {
    match value {
        Value::String(value) => value.zeroize(),
        Value::Array(values) => values.iter_mut().for_each(erase_json),
        Value::Object(values) => values.values_mut().for_each(erase_json),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        body::Body,
        extract::State as WebState,
        http::{Response, StatusCode},
        routing::post,
    };
    use std::sync::{Arc, Mutex as StdMutex};
    use tokio::sync::Notify;

    struct Reply {
        status: StatusCode,
        body: Value,
        gate: Option<Arc<Notify>>,
    }
    impl From<Value> for Reply {
        fn from(body: Value) -> Self {
            Self {
                status: StatusCode::OK,
                body,
                gate: None,
            }
        }
    }
    #[derive(Default)]
    struct FakeState {
        replies: StdMutex<VecDeque<Reply>>,
        requests: StdMutex<Vec<(HeaderMap, Value)>>,
        received: Notify,
    }
    struct Fake {
        client: Arc<LiveClient>,
        state: Arc<FakeState>,
        task: tokio::task::JoinHandle<()>,
        store: Arc<vault::tests::MemoryStore>,
    }
    impl Drop for Fake {
        fn drop(&mut self) {
            self.task.abort();
        }
    }
    async fn handler(
        WebState(state): WebState<Arc<FakeState>>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Response<Body> {
        state.requests.lock().unwrap().push((headers, body));
        state.received.notify_waiters();
        let reply = state
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("Unexpected network request");
        if let Some(gate) = reply.gate {
            gate.notified().await;
        }
        Response::builder()
            .status(reply.status)
            .header("location", "/redirected-target-that-must-not-be-requested")
            .header("content-type", "application/json")
            .body(Body::from(reply.body.to_string()))
            .unwrap()
    }
    async fn new_fake(replies: Vec<Reply>) -> Fake {
        new_fake_with_store(replies, Arc::new(vault::tests::MemoryStore::default())).await
    }
    async fn new_fake_with_store(
        replies: Vec<Reply>,
        store: Arc<vault::tests::MemoryStore>,
    ) -> Fake {
        let state = Arc::new(FakeState::default());
        *state.replies.lock().unwrap() = replies.into();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = Router::new()
            .route("/graphql", post(handler))
            .with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let client = LiveClient {
            http: LiveClient::http_builder().build().unwrap(),
            state: Arc::new(Mutex::new(State::default())),
            epoch: watch::channel(0).0,
            revoked: AtomicBool::new(false),
            vault: vault::Vault::with_store(store.clone()),
            test_endpoint: Some(format!("http://{addr}/graphql")),
        };
        Fake {
            client: Arc::new(client),
            state,
            task,
            store,
        }
    }
    fn csrf() -> Value {
        json!({"data":{"createCsrfToken":{"csrfToken":"csrf-secret","appSessionToken":"app-secret"}}})
    }
    fn authenticated() -> Value {
        json!({"data":{"login":{"__typename":"MobileLoginResponse","userSessionToken":"user-secret"}}})
    }
    fn account() -> Value {
        json!({"data":{"currentUser":{"id":"account-1","firstName":"Owner","lastName":"Test","vehicles":[{"id":"vehicle-1","name":"Truck","vehicle":{"model":"R1T","modelYear":2025}}]}}})
    }
    fn mfa() -> Value {
        json!({"data":{"login":{"__typename":"MobileMFALoginResponse","otpToken":"otp-secret","targetChannel":{"__typename":"MfaAuthenticatorChannel"}}}})
    }
    async fn clear_cooldown(client: &LiveClient) {
        client.state.lock().await.last_auth_attempt = None;
    }
    async fn wait_requests(fake: &Fake, count: usize) {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let notify = fake.state.received.notified();
                tokio::pin!(notify);
                notify.as_mut().enable();
                if fake.state.requests.lock().unwrap().len() >= count {
                    break;
                }
                notify.await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn login_and_live_query_keep_tokens_native_and_preserve_missing_readings() {
        let fake = new_fake(vec![csrf().into(), authenticated().into(), account().into(), json!({"data":{"vehicleState":{"batteryLevel":{"value":72,"timeStamp":"2026-10-09T00:00:00Z"},"password":"should-never-escape","nested":{"accessToken":"also-secret"}}}}).into()]).await;
        let status = fake
            .client
            .login("owner@example.com", "password-secret")
            .await
            .unwrap();
        assert_eq!(status.state, AuthState::Authenticated);
        assert!(!serde_json::to_string(&status).unwrap().contains("secret"));
        let response = fake
            .client
            .execute("vehicle-state", &json!({"vehicle_id":"vehicle-1"}))
            .await
            .unwrap();
        assert_eq!(response["source"], "rivian");
        assert_eq!(response["synthetic"], false);
        assert!(response["observed_at"].is_null());
        assert_eq!(response["data"]["vehicle"]["battery_percent"], 72.0);
        assert!(response["data"]["vehicle"]["odometer_km"].is_null());
        assert_eq!(
            response["raw_response"]["data"]["vehicleState"]["password"],
            "[redacted]"
        );
        assert!(!response.to_string().contains("should-never-escape"));
        let requests = fake.state.requests.lock().unwrap();
        assert!(requests[0].0.get("u-sess").is_none());
        assert_eq!(requests[1].0["a-sess"], "app-secret");
        assert_eq!(requests[1].1["variables"]["password"], "password-secret");
        assert_eq!(requests[2].0["u-sess"], "user-secret");
        assert_eq!(
            requests[2].0["apollographql-client-name"],
            "com.rivian.android.consumer"
        );
    }

    #[tokio::test]
    async fn mfa_uses_v2_and_rejects_codes_without_disclosing_challenge() {
        let fake = new_fake(vec![csrf().into(), mfa().into(), json!({"data":{"loginWithOTPV2":{"__typename":"MobileLoginResponse","userSessionToken":"user-secret"}}}).into(), account().into()]).await;
        let status = fake
            .client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        assert_eq!(status.state, AuthState::MfaRequired);
        assert!(
            !serde_json::to_string(&status)
                .unwrap()
                .contains("otp-secret")
        );
        assert_eq!(
            fake.client.verify_otp("bad-code").await.unwrap_err().code(),
            "invalid_input"
        );
        clear_cooldown(&fake.client).await;
        assert_eq!(
            fake.client.verify_otp("123456").await.unwrap().state,
            AuthState::Authenticated
        );
        let requests = fake.state.requests.lock().unwrap();
        assert!(
            requests[2].1["query"]
                .as_str()
                .unwrap()
                .contains("loginWithOTPV2")
        );
        assert_eq!(requests[2].1["variables"]["otpToken"], "otp-secret");
    }

    #[tokio::test]
    async fn wrong_vehicle_and_arbitrary_operations_never_dispatch() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        assert_eq!(
            fake.client
                .execute(
                    "vehicle-state",
                    &json!({"vehicle_id":"someone-elses-vehicle"})
                )
                .await
                .unwrap_err()
                .code(),
            "vehicle_not_owned"
        );
        assert_eq!(
            fake.client
                .execute(
                    "vehicle-state",
                    &json!({"vehicle_id":"vehicle-1","query":"mutation DeleteEverything"})
                )
                .await
                .unwrap_err()
                .code(),
            "invalid_operation"
        );
        assert_eq!(
            fake.client
                .execute("unlock-vehicle", &json!({"vehicle_id":"vehicle-1"}))
                .await
                .unwrap_err()
                .code(),
            "invalid_operation"
        );
        assert_eq!(fake.state.requests.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn expired_user_session_rotates_once_then_requires_sign_in() {
        let denied = json!({"errors":[{"extensions":{"code":"UNAUTHENTICATED"},"message":"secret diagnostic"}],"data":{"vehicleState":null}});
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            denied.clone().into(),
            csrf().into(),
            denied.into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let error = fake
            .client
            .execute("vehicle-state", &json!({"vehicle_id":"vehicle-1"}))
            .await
            .unwrap_err();
        assert_eq!(error.code(), "account_auth_required");
        assert!(!format!("{error:?}").contains("secret diagnostic"));
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        assert_eq!(fake.state.requests.lock().unwrap().len(), 6);
    }

    #[tokio::test]
    async fn rate_limit_pauses_following_requests_without_automatic_retry() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            Reply {
                status: StatusCode::TOO_MANY_REQUESTS,
                body: json!({}),
                gate: None,
            },
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        for _ in 0..2 {
            assert_eq!(
                fake.client
                    .execute("vehicle-state", &json!({"vehicle_id":"vehicle-1"}))
                    .await
                    .unwrap_err()
                    .code(),
                "rate_limited"
            );
        }
        assert_eq!(fake.state.requests.lock().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn malformed_mfa_result_clears_pending_secrets() {
        let fake = new_fake(vec![
            csrf().into(),
            mfa().into(),
            json!({"data":{"loginWithOTPV2":{}}}).into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        clear_cooldown(&fake.client).await;
        assert_eq!(
            fake.client.verify_otp("123456").await.unwrap_err().code(),
            "upstream_schema"
        );
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        assert!(fake.client.state.lock().await.pending.is_none());
    }

    #[tokio::test]
    async fn fifth_rejected_otp_erases_the_challenge_immediately() {
        let fake = new_fake(vec![
            csrf().into(),
            mfa().into(),
            json!({"data":{},"errors":[{"extensions":{"code":"INVALID_OTP"}}]}).into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        clear_cooldown(&fake.client).await;
        fake.client
            .state
            .lock()
            .await
            .pending
            .as_mut()
            .unwrap()
            .attempts = 4;
        assert_eq!(
            fake.client.verify_otp("123456").await.unwrap_err().code(),
            "invalid_otp"
        );
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        assert!(fake.client.state.lock().await.pending.is_none());
    }

    #[tokio::test]
    async fn remembering_saves_only_native_tokens_and_identity_after_validation() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
        ])
        .await;
        fake.client.remember(true).await.unwrap();
        fake.client
            .login("owner@example.com", "password-secret")
            .await
            .unwrap();
        let bytes = fake.store.read_for_test().unwrap();
        let stored: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(stored["user_session"], "user-secret");
        assert_eq!(stored["user_id"], "account-1");
        assert!(stored.get("password").is_none());
        assert!(stored.get("email").is_none());
        assert!(stored.get("otp_token").is_none());
        assert!(fake.client.vault_status().await.stored);
        fake.client.revoke();
        assert!(
            fake.store.read_for_test().is_some(),
            "process/capability stop preserves opt-in"
        );
        fake.client.logout().await;
        assert!(
            fake.store.read_for_test().is_none(),
            "explicit sign-out clears after revoke"
        );
    }

    #[tokio::test]
    async fn remembered_restore_rechecks_identity_and_fresh_owned_vehicles() {
        let original = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
        ])
        .await;
        original.client.remember(true).await.unwrap();
        original
            .client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let next = new_fake_with_store(
            vec![csrf().into(), account().into()],
            original.store.clone(),
        )
        .await;
        assert_eq!(next.client.status().await.state, AuthState::SignedOut);
        assert_eq!(
            next.client.restore_remembered().await.unwrap().state,
            AuthState::Authenticated
        );
        assert!(next.client.check_vehicle_access("vehicle-1").await.is_ok());
        let requests = next.state.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].0.get("u-sess").is_none());
        assert_eq!(requests[1].0["u-sess"], "user-secret");
    }

    #[tokio::test]
    async fn remembered_restore_rejects_changed_account_identity() {
        let original = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
        ])
        .await;
        original.client.remember(true).await.unwrap();
        original
            .client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let mut other_account = account();
        other_account["data"]["currentUser"]["id"] = json!("different-account");
        let next = new_fake_with_store(
            vec![csrf().into(), other_account.into()],
            original.store.clone(),
        )
        .await;
        assert_eq!(
            next.client.restore_remembered().await.unwrap_err().code(),
            "upstream_schema"
        );
        assert_eq!(next.client.status().await.state, AuthState::SignedOut);
        assert!(next.store.read_for_test().is_none());
    }

    #[tokio::test]
    async fn unavailable_vault_fails_closed_while_memory_sign_in_works() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
        ])
        .await;
        fake.store.unavailable.store(true, Ordering::SeqCst);
        assert_eq!(
            fake.client.remember(true).await.unwrap_err().code(),
            "vault_unavailable"
        );
        assert_eq!(
            fake.client
                .login("owner@example.com", "password")
                .await
                .unwrap()
                .state,
            AuthState::Authenticated
        );
        assert!(!fake.client.vault_status().await.remembering);
        assert!(fake.store.read_for_test().is_none());
    }

    #[tokio::test]
    async fn malformed_or_expired_remembered_bundle_never_dispatches() {
        for bundle in [
            json!({"password":"must-be-erased"}),
            json!({"version":1,"saved_at":0,"csrf":"csrf-secret","app_session":"app-secret","user_session":"user-secret","user_id":"account-1"}),
        ] {
            let fake = new_fake(vec![]).await;
            fake.store
                .replace_for_test(serde_json::to_vec(&bundle).unwrap());
            assert_eq!(
                fake.client.restore_remembered().await.unwrap_err().code(),
                "remembered_session_invalid"
            );
            assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
            assert!(fake.store.read_for_test().is_none());
            assert!(fake.state.requests.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn logout_cancels_restore_without_restoring_a_vault_secret() {
        let original = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
        ])
        .await;
        original.client.remember(true).await.unwrap();
        original
            .client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let gate = Arc::new(Notify::new());
        let next = new_fake_with_store(
            vec![
                csrf().into(),
                Reply {
                    status: StatusCode::OK,
                    body: account(),
                    gate: Some(gate.clone()),
                },
            ],
            original.store.clone(),
        )
        .await;
        let client = next.client.clone();
        let restore = tokio::spawn(async move { client.restore_remembered().await });
        wait_requests(&next, 2).await;
        next.client.logout().await;
        assert_eq!(
            restore.await.unwrap().unwrap_err().code(),
            "session_revoked"
        );
        gate.notify_waiters();
        assert_eq!(next.client.status().await.state, AuthState::SignedOut);
        assert!(next.store.read_for_test().is_none());
    }

    #[tokio::test]
    async fn aborted_login_vault_write_cannot_resurrect_after_logout() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            json!({"data":{"logout":{"success":true}}}).into(),
        ])
        .await;
        fake.client.remember(true).await.unwrap();
        let gate = Arc::new(vault::tests::WriteGate::default());
        *fake.store.write_gate.lock().unwrap() = Some(gate.clone());
        let client = fake.client.clone();
        let login =
            tokio::spawn(async move { client.login("owner@example.com", "password").await });
        tokio::time::timeout(Duration::from_secs(2), gate.entered.notified())
            .await
            .unwrap();
        login.abort();
        assert!(login.await.unwrap_err().is_cancelled());
        let epoch = fake.client.authorization_epoch();
        let client = fake.client.clone();
        let logout = tokio::spawn(async move { client.logout().await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while fake.client.authorization_epoch() == epoch {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        gate.release();
        tokio::time::timeout(Duration::from_secs(2), logout)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        assert!(fake.store.read_for_test().is_none());
    }

    #[tokio::test]
    async fn vault_delete_failure_is_reported_while_memory_is_erased() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            json!({"data":{"logout":{"success":true}}}).into(),
        ])
        .await;
        fake.client.remember(true).await.unwrap();
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        fake.store.delete_fails.store(true, Ordering::SeqCst);
        assert!(
            fake.client.logout().await,
            "remote-confirmed result is independent of local deletion"
        );
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        let vault = fake.client.vault_status().await;
        assert!(!vault.remembering);
        assert!(vault.stored);
        assert_eq!(vault.error, Some("vault_delete_failed"));
        fake.store.delete_fails.store(false, Ordering::SeqCst);
        fake.client.logout().await;
        assert!(fake.store.read_for_test().is_none());
    }

    #[tokio::test]
    async fn native_mfa_expiry_erases_challenge_without_status_or_another_request() {
        let fake = new_fake(vec![csrf().into(), mfa().into()]).await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let created = Instant::now() - MFA_LIFETIME + Duration::from_millis(15);
        fake.client
            .state
            .lock()
            .await
            .pending
            .as_mut()
            .unwrap()
            .created = created;
        let active = fake
            .client
            .state
            .lock()
            .await
            .pending
            .as_ref()
            .unwrap()
            .active
            .clone();
        fake.client
            .schedule_mfa_expiry(fake.client.authorization_epoch(), created, active);
        tokio::time::sleep(Duration::from_millis(40)).await;
        let state = fake.client.state.lock().await;
        assert!(state.pending.is_none());
        assert!(state.csrf.is_none());
        assert!(state.app_session.is_none());
        assert_eq!(fake.state.requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn transport_reauthentication_clears_memory_vault_and_subscriptions() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
        ])
        .await;
        fake.client.remember(true).await.unwrap();
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let credentials = fake
            .client
            .subscription_credentials("vehicle-1")
            .await
            .unwrap();
        assert_eq!(
            fake.client.require_sign_in().await.state,
            AuthState::SignedOut
        );
        assert!(!credentials.is_current());
        assert!(fake.store.read_for_test().is_none());
        assert_eq!(fake.state.requests.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn delayed_authentication_failure_cannot_erase_a_newer_validated_account() {
        let mut newer = account();
        newer["data"]["currentUser"]["id"] = json!("account-2");
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            csrf().into(),
            authenticated().into(),
            newer.into(),
        ])
        .await;
        fake.client.remember(true).await.unwrap();
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let original = fake.client.authorization_epoch();
        let invalidation_epoch = fake.client.advance_epoch_if_current(original).unwrap();
        // Defer the invalidation continuation, just as a queued state mutex or
        // thread scheduling can defer it after its atomic compare-and-increment.
        clear_cooldown(&fake.client).await;
        fake.client
            .login("second@example.com", "password")
            .await
            .unwrap();
        assert_eq!(
            fake.client
                .finish_account_invalidation(invalidation_epoch)
                .await
                .state,
            AuthState::Authenticated
        );
        let stored: Value = serde_json::from_slice(&fake.store.read_for_test().unwrap()).unwrap();
        assert_eq!(stored["user_id"], "account-2");
        assert_eq!(fake.state.requests.lock().unwrap().len(), 6);
    }

    #[tokio::test]
    async fn delayed_logout_cannot_delete_a_newer_validated_account() {
        let mut newer = account();
        newer["data"]["currentUser"]["id"] = json!("account-2");
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            csrf().into(),
            authenticated().into(),
            newer.into(),
        ])
        .await;
        fake.client.remember(true).await.unwrap();
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let logout_epoch = fake.client.advance_epoch();
        clear_cooldown(&fake.client).await;
        fake.client
            .login("second@example.com", "password")
            .await
            .unwrap();
        assert!(!fake.client.finish_logout(logout_epoch).await);
        assert_eq!(fake.client.status().await.state, AuthState::Authenticated);
        let stored: Value = serde_json::from_slice(&fake.store.read_for_test().unwrap()).unwrap();
        assert_eq!(stored["user_id"], "account-2");
        assert_eq!(fake.state.requests.lock().unwrap().len(), 6);
    }

    #[tokio::test]
    async fn new_generation_cannot_adopt_previous_account_credentials_before_login() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let transition_epoch = fake.client.advance_epoch();
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        assert_eq!(
            fake.client
                .subscription_credentials_at_epoch("vehicle-1", transition_epoch)
                .await
                .err()
                .unwrap()
                .code(),
            "account_auth_required"
        );
        assert_eq!(
            fake.client
                .execute_read_at_epoch(
                    "vehicle-state",
                    &json!({"vehicle_id":"vehicle-1"}),
                    transition_epoch
                )
                .await
                .unwrap_err()
                .code(),
            "account_auth_required"
        );
        assert_eq!(fake.state.requests.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn subscription_capabilities_require_owned_vehicle_and_observe_logout() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            json!({"data":{"logout":{"success":true}}}).into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        assert_eq!(
            fake.client
                .subscription_credentials("not-owned")
                .await
                .err()
                .unwrap()
                .code(),
            "vehicle_not_owned"
        );
        let credentials = fake
            .client
            .subscription_credentials("vehicle-1")
            .await
            .unwrap();
        assert!(credentials.is_current());
        assert_eq!(credentials.vehicle_id(), "vehicle-1");
        assert!(fake.client.logout().await);
        assert!(!credentials.is_current());
        assert_eq!(fake.state.requests.lock().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn explicit_second_login_revokes_old_vehicle_capability_and_stored_identity() {
        let mut changed = account();
        changed["data"]["currentUser"]["id"] = json!("account-2");
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            csrf().into(),
            authenticated().into(),
            changed.into(),
        ])
        .await;
        fake.client.remember(true).await.unwrap();
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let credentials = fake
            .client
            .subscription_credentials("vehicle-1")
            .await
            .unwrap();
        let epoch = fake.client.authorization_epoch();
        clear_cooldown(&fake.client).await;
        fake.client
            .login("second@example.com", "second-password")
            .await
            .unwrap();
        assert_ne!(fake.client.authorization_epoch(), epoch);
        assert!(!credentials.is_current());
        let stored: Value = serde_json::from_slice(&fake.store.read_for_test().unwrap()).unwrap();
        assert_eq!(stored["user_id"], "account-2");
    }

    #[tokio::test]
    async fn failed_explicit_login_cannot_restore_previously_remembered_account() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            csrf().into(),
            json!({"data":{},"errors":[{"extensions":{"code":"INVALID_CREDENTIALS"}}]}).into(),
        ])
        .await;
        fake.client.remember(true).await.unwrap();
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        clear_cooldown(&fake.client).await;
        assert_eq!(
            fake.client
                .login("second@example.com", "incorrect-password")
                .await
                .unwrap_err()
                .code(),
            "invalid_credentials"
        );
        assert!(fake.store.read_for_test().is_none());
        assert_eq!(
            fake.client.restore_remembered().await.unwrap().state,
            AuthState::SignedOut
        );
        assert_eq!(fake.state.requests.lock().unwrap().len(), 5);
    }

    #[tokio::test]
    async fn explicit_login_cancels_an_older_inflight_read_before_mutex_acquisition() {
        let gate = Arc::new(Notify::new());
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            Reply {
                status: StatusCode::OK,
                body: json!({"data":{"vehicleState":{"batteryLevel":{"value":1}}}}),
                gate: Some(gate.clone()),
            },
            csrf().into(),
            authenticated().into(),
            account().into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        clear_cooldown(&fake.client).await;
        let client = fake.client.clone();
        let read = tokio::spawn(async move {
            client
                .execute("vehicle-state", &json!({"vehicle_id":"vehicle-1"}))
                .await
        });
        wait_requests(&fake, 4).await;
        let client = fake.client.clone();
        let login =
            tokio::spawn(async move { client.login("owner@example.com", "password").await });
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), read)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err()
                .code(),
            "session_revoked"
        );
        gate.notify_waiters();
        assert_eq!(
            login.await.unwrap().unwrap().state,
            AuthState::Authenticated
        );
    }

    #[tokio::test]
    async fn resend_otp_is_fixed_bounded_and_never_returns_challenge_secrets() {
        let fake = new_fake(vec![
            csrf().into(),
            mfa().into(),
            json!({"data":{"resendOTPCode":{"success":true}}}).into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        clear_cooldown(&fake.client).await;
        let status = fake.client.resend_otp().await.unwrap();
        assert_eq!(status.state, AuthState::MfaRequired);
        assert!(
            !serde_json::to_string(&status)
                .unwrap()
                .contains("otp-secret")
        );
        assert_eq!(
            fake.client.resend_otp().await.unwrap_err().code(),
            "rate_limited"
        );
        let requests = fake.state.requests.lock().unwrap();
        assert_eq!(requests[2].1["operationName"], "ResendOTPCode");
        assert_eq!(requests[2].1["variables"], json!({"otpToken":"otp-secret"}));
        assert_eq!(requests.len(), 3);
    }

    #[tokio::test]
    async fn order_authority_is_fetched_natively_before_order_dispatch() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            json!({"data":{"user":{"orderSnapshots":[{"id":"owned-order"}]}}}).into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let error = fake
            .client
            .execute("order-details", &json!({"order_id":"someone-elses-order"}))
            .await
            .unwrap_err();
        assert_eq!(error.code(), "resource_not_owned");
        let requests = fake.state.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(requests[3].1["operationName"], "MyRivianDataOrders");
        assert!(requests[3].1["variables"].as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn wallbox_registration_must_match_signed_in_user() {
        let fake = new_fake(vec![csrf().into(), authenticated().into(), account().into(),
            json!({"data":{"getRegisteredWallboxes":[{"wallboxId":"wallbox-1","userId":"another-owner"}]}}).into()]).await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        assert_eq!(
            fake.client
                .execute("wallbox-status", &json!({"wallbox_id":"wallbox-1"}))
                .await
                .unwrap_err()
                .code(),
            "resource_not_owned"
        );
        let requests = fake.state.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(requests[3].1["operationName"], "getRegisteredWallboxes");
    }

    #[tokio::test]
    async fn confirmed_write_never_retries_after_authentication_rejection() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            json!({"data":{},"errors":[{"extensions":{"code":"UNAUTHENTICATED"}}]}).into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        assert_eq!(
            fake.client
                .execute_confirmed(
                    "rename-vehicle",
                    &json!({"vehicle_id":"vehicle-1","name":"New name"}),
                    fake.client.authorization_epoch(),
                )
                .await
                .unwrap_err()
                .code(),
            "account_auth_required"
        );
        assert_eq!(fake.state.requests.lock().unwrap().len(), 4);
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
    }

    #[tokio::test]
    async fn stale_confirmation_cannot_dispatch_for_a_new_account_with_same_vehicle_id() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            csrf().into(),
            authenticated().into(),
            account().into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let expected_epoch = fake.client.authorization_epoch();
        clear_cooldown(&fake.client).await;
        fake.client
            .login("second@example.com", "second-password")
            .await
            .unwrap();
        let variables = json!({"vehicle_id":"vehicle-1","name":"Old account confirmation"});
        assert_eq!(
            fake.client
                .execute_confirmed("rename-vehicle", &variables, expected_epoch)
                .await
                .unwrap_err()
                .code(),
            "session_revoked"
        );
        assert_eq!(
            fake.client
                .execute("rename-vehicle", &variables)
                .await
                .unwrap_err()
                .code(),
            "confirmation_required"
        );
        assert_eq!(fake.state.requests.lock().unwrap().len(), 6);
    }

    #[tokio::test]
    async fn logout_during_preliminary_remember_io_cannot_admit_a_new_login() {
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            json!({"data":{"logout":{"success":true}}}).into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        clear_cooldown(&fake.client).await;
        let gate = Arc::new(vault::tests::WriteGate::default());
        *fake.store.write_gate.lock().unwrap() = Some(gate.clone());
        let expected_epoch = fake.client.authorization_epoch();
        let client = fake.client.clone();
        let login = tokio::spawn(async move {
            client
                .login_with_remember(
                    "second@example.com",
                    "second-password",
                    true,
                    expected_epoch,
                )
                .await
        });
        tokio::time::timeout(Duration::from_secs(2), gate.entered.notified())
            .await
            .unwrap();
        let client = fake.client.clone();
        let logout = tokio::spawn(async move { client.logout().await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while fake.client.authorization_epoch() == expected_epoch {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        gate.release();
        assert_eq!(login.await.unwrap().unwrap_err().code(), "session_revoked");
        logout.await.unwrap();
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        assert!(fake.store.read_for_test().is_none());
        assert_eq!(fake.state.requests.lock().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn permanent_revoke_cancels_inflight_login_and_queued_requests() {
        let gate = Arc::new(Notify::new());
        let fake = new_fake(vec![Reply {
            status: StatusCode::OK,
            body: csrf(),
            gate: Some(gate.clone()),
        }])
        .await;
        let client = fake.client.clone();
        let login =
            tokio::spawn(async move { client.login("owner@example.com", "password").await });
        wait_requests(&fake, 1).await;
        let client = fake.client.clone();
        let admitted_epoch = client.authorization_epoch();
        let queued = tokio::spawn(async move {
            client
                .login_with_remember("owner@example.com", "password", false, admitted_epoch)
                .await
        });
        tokio::task::yield_now().await;
        fake.client.revoke();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), login)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err()
                .code(),
            "session_revoked"
        );
        assert_eq!(queued.await.unwrap().unwrap_err().code(), "session_revoked");
        gate.notify_waiters();
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        assert_eq!(fake.state.requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn logout_cancels_pending_mfa_then_allows_new_sign_in() {
        let gate = Arc::new(Notify::new());
        let fake = new_fake(vec![
            csrf().into(),
            mfa().into(),
            Reply {
                status: StatusCode::OK,
                body: json!({"data":{"loginWithOTPV2":{"userSessionToken":"late-secret"}}}),
                gate: Some(gate.clone()),
            },
            csrf().into(),
            authenticated().into(),
            account().into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        clear_cooldown(&fake.client).await;
        let client = fake.client.clone();
        let verify = tokio::spawn(async move { client.verify_otp("123456").await });
        wait_requests(&fake, 3).await;
        assert!(fake.client.logout().await);
        assert_eq!(verify.await.unwrap().unwrap_err().code(), "session_revoked");
        gate.notify_waiters();
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        clear_cooldown(&fake.client).await;
        assert_eq!(
            fake.client
                .login("owner@example.com", "new-password")
                .await
                .unwrap()
                .state,
            AuthState::Authenticated
        );
    }

    #[tokio::test]
    async fn response_bounds_and_redirects_fail_closed() {
        let fake = new_fake(vec![
            json!({"data":{"huge":"x".repeat(MAX_RESPONSE_BYTES)}}).into(),
        ])
        .await;
        assert_eq!(
            fake.client
                .login("owner@example.com", "password")
                .await
                .unwrap_err()
                .code(),
            "response_too_large"
        );
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        let fake = new_fake(vec![Reply {
            status: StatusCode::TEMPORARY_REDIRECT,
            body: json!({}),
            gate: None,
        }])
        .await;
        assert_eq!(
            fake.client
                .login("owner@example.com", "password")
                .await
                .unwrap_err()
                .code(),
            "upstream_unavailable"
        );
        assert_eq!(fake.state.requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn login_is_throttled_without_repeating_upstream_credentials() {
        let fake = new_fake(vec![csrf().into(), json!({"errors":[{"extensions":{"code":"BAD_CURRENT_PASSWORD"},"message":"do-not-return"}]}).into()]).await;
        let error = fake
            .client
            .login("owner@example.com", "password")
            .await
            .unwrap_err();
        assert_eq!(error.code(), "invalid_credentials");
        assert!(!error.message().contains("do-not-return"));
        assert_eq!(
            fake.client
                .login("owner@example.com", "password")
                .await
                .unwrap_err()
                .code(),
            "rate_limited"
        );
        assert_eq!(fake.state.requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn changed_account_identity_requires_new_explicit_login() {
        let mut changed = account();
        changed["data"]["currentUser"]["id"] = json!("different-account");
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            changed.into(),
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        assert_eq!(
            fake.client.vehicles().await.unwrap_err().code(),
            "upstream_schema"
        );
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        assert!(fake.client.state.lock().await.vehicle_ids.is_empty());
    }

    #[tokio::test]
    async fn revoke_cancels_live_read_without_releasing_late_response() {
        let gate = Arc::new(Notify::new());
        let fake = new_fake(vec![
            csrf().into(),
            authenticated().into(),
            account().into(),
            Reply {
                status: StatusCode::OK,
                body: json!({"data":{"vehicleState":{"batteryLevel":{"value":99}}}}),
                gate: Some(gate.clone()),
            },
        ])
        .await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        let client = fake.client.clone();
        let read = tokio::spawn(async move {
            client
                .execute("vehicle-state", &json!({"vehicle_id":"vehicle-1"}))
                .await
        });
        wait_requests(&fake, 4).await;
        fake.client.revoke();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), read)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err()
                .code(),
            "session_revoked"
        );
        gate.notify_waiters();
        let state = fake.client.state.lock().await;
        assert!(state.user_session.is_none());
        assert!(state.vehicle_ids.is_empty());
    }

    #[tokio::test]
    async fn mfa_expiry_and_attempt_cap_erase_challenge_without_network() {
        let fake = new_fake(vec![csrf().into(), mfa().into()]).await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        {
            let mut state = fake.client.state.lock().await;
            state.last_auth_attempt = None;
            state.pending.as_mut().unwrap().attempts = 5;
        }
        assert_eq!(
            fake.client.verify_otp("123456").await.unwrap_err().code(),
            "mfa_expired"
        );
        assert!(fake.client.state.lock().await.pending.is_none());
        assert_eq!(fake.state.requests.lock().unwrap().len(), 2);
        let fake = new_fake(vec![csrf().into(), mfa().into()]).await;
        fake.client
            .login("owner@example.com", "password")
            .await
            .unwrap();
        fake.client
            .state
            .lock()
            .await
            .pending
            .as_mut()
            .unwrap()
            .created = Instant::now() - MFA_LIFETIME;
        assert_eq!(fake.client.status().await.state, AuthState::SignedOut);
        assert!(fake.client.state.lock().await.csrf.is_none());
    }

    #[test]
    fn malformed_graphql_errors_are_not_success_and_redaction_is_recursive() {
        assert_eq!(
            graphql_error(&json!({"data":{},"errors":{"message":"secret"}}))
                .unwrap()
                .code(),
            "upstream_schema"
        );
        let mut value = json!({"nested":[{"refresh_token":"s","userSessionToken":"s","pin":"1234","name":"Truck"}]});
        redact_secrets(&mut value);
        assert_eq!(value["nested"][0]["refresh_token"], "[redacted]");
        assert_eq!(value["nested"][0]["userSessionToken"], "[redacted]");
        assert_eq!(value["nested"][0]["pin"], "[redacted]");
        assert_eq!(value["nested"][0]["name"], "Truck");
        assert!(secret_field(&json!({"token":"line\r\nbreak"}), "token").is_err());
    }
}
