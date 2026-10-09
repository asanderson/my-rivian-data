use axum::{
    Router,
    body::Body,
    http::{Request, Response, StatusCode, header},
};
use http_body_util::BodyExt;
use rivian_host::{AppState, app};
use serde_json::{Value, json};
use std::time::Duration;
use tower::ServiceExt;

const HOST: &str = "127.0.0.1:8765";
const ORIGIN: &str = "http://127.0.0.1:8765";

fn request(method: &str, path: &str, body: Value) -> axum::http::request::Builder {
    let _ = body;
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "application/json")
}

async fn payload(response: Response<Body>) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

async fn login(router: &Router, token: &str) -> (String, String, String) {
    let response = router
        .clone()
        .oneshot(
            request("POST", "/api/bootstrap", json!({}))
                .header(header::ORIGIN, ORIGIN)
                .body(Body::from(json!({"token": token}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie_header = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie_header.contains("HttpOnly"));
    assert!(cookie_header.contains("SameSite=Strict"));
    let cookie = cookie_header.split(';').next().unwrap().to_string();
    let body = payload(response).await;
    let csrf = body["csrf_token"].as_str().unwrap().to_string();
    let capability = body["app_capability"].as_str().unwrap().to_string();
    assert_eq!(capability.len(), 64);
    (cookie, csrf, capability)
}

#[tokio::test]
async fn authentication_and_security_headers_cover_api_and_errors() {
    let (state, token) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    let response = router
        .clone()
        .oneshot(
            request("GET", "/api/catalog", json!({}))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    let csp = response.headers()[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap();
    assert!(csp.contains("'wasm-unsafe-eval'"));
    assert!(!csp.contains("'unsafe-eval'"));
    assert!(
        !response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    let (cookie, _, capability) = login(&router, &token).await;
    let response = router
        .oneshot(
            request("GET", "/api/catalog", json!({}))
                .header(header::COOKIE, cookie)
                .header("x-app-capability", &capability)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(payload(response).await.as_array().unwrap().len(), 10);
}

#[tokio::test]
async fn bootstrap_is_single_use_even_for_concurrent_requests() {
    let (state, token) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    let make_request = || {
        request("POST", "/api/bootstrap", json!({}))
            .header(header::ORIGIN, ORIGIN)
            .body(Body::from(json!({"token": token}).to_string()))
            .unwrap()
    };
    let (a, b) = tokio::join!(
        router.clone().oneshot(make_request()),
        router.oneshot(make_request())
    );
    let mut statuses = [a.unwrap().status().as_u16(), b.unwrap().status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 401]);
}

#[tokio::test]
async fn incorrect_bootstrap_does_not_consume_valid_capability() {
    let (state, token) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    let response = router
        .clone()
        .oneshot(
            request("POST", "/api/bootstrap", json!({}))
                .header(header::ORIGIN, ORIGIN)
                .body(Body::from(json!({"token":"incorrect"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    login(&router, &token).await;
}

#[tokio::test]
async fn bootstrap_expiry_and_session_expiry_are_enforced() {
    let (state, token) = AppState::with_lifetimes(
        8765,
        Duration::ZERO,
        Duration::from_secs(60),
        Duration::from_secs(60),
    )
    .unwrap();
    let response = app(state)
        .oneshot(
            request("POST", "/api/bootstrap", json!({}))
                .header(header::ORIGIN, ORIGIN)
                .body(Body::from(json!({"token":token}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    for (idle, absolute) in [
        (Duration::ZERO, Duration::from_secs(60)),
        (Duration::from_secs(60), Duration::ZERO),
    ] {
        let (state, token) =
            AppState::with_lifetimes(8765, Duration::from_secs(60), idle, absolute).unwrap();
        let router = app(state);
        let (cookie, _, capability) = login(&router, &token).await;
        let response = router
            .oneshot(
                request("GET", "/api/session", json!({}))
                    .header(header::COOKIE, cookie)
                    .header("x-app-capability", &capability)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn host_origin_and_fetch_metadata_cannot_bypass_boundary() {
    let (state, token) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    for host in [
        "localhost:8765",
        "127.0.0.1:8766",
        "127.0.0.1.attacker.example:8765",
        "attacker.example",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .header(header::HOST, host)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    for origin in [
        None,
        Some("null"),
        Some("https://attacker.example"),
        Some("http://127.0.0.1:9999"),
    ] {
        let mut builder = request("POST", "/api/bootstrap", json!({}));
        if let Some(origin) = origin {
            builder = builder.header(header::ORIGIN, origin);
        }
        let response = router
            .clone()
            .oneshot(
                builder
                    .body(Body::from(json!({"token":token}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    for site in ["cross-site", "same-site"] {
        let response = router
            .clone()
            .oneshot(
                request("GET", "/api/health", json!({}))
                    .header("sec-fetch-site", site)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn csrf_is_required_for_execution_and_logout_and_logout_revokes() {
    let (state, token) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    let (cookie, csrf, capability) = login(&router, &token).await;
    for path in ["/api/execute", "/api/logout"] {
        let response = router
            .clone()
            .oneshot(
                request("POST", path, json!({}))
                    .header(header::ORIGIN, ORIGIN)
                    .header(header::COOKIE, &cookie)
                    .header("x-app-capability", &capability)
                    .body(Body::from(
                        json!({"operation_id":"list-vehicles","variables":{}}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let response = router
        .clone()
        .oneshot(
            request("POST", "/api/execute", json!({}))
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, &cookie)
                .header("x-app-capability", &capability)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({"operation_id":"list-vehicles","variables":{}}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(payload(response).await["mode"], "demo");
    let response = router
        .clone()
        .oneshot(
            request("POST", "/api/logout", json!({}))
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, &cookie)
                .header("x-app-capability", &capability)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    let response = router
        .oneshot(
            request("GET", "/api/vehicles", json!({}))
                .header(header::COOKIE, cookie)
                .header("x-app-capability", &capability)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn native_execution_rejects_unsafe_unknown_or_unauthorized_operations() {
    let (state, token) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    let (cookie, csrf, capability) = login(&router, &token).await;
    for body in [
        json!({"operation_id":"unlock-vehicle","variables":{"vehicle_id":"demo-r1t-001"}}),
        json!({"operation_id":"vehicle-state","variables":{"vehicle_id":"another-account-vehicle"}}),
        json!({"operation_id":"http://attacker.example","variables":{}}),
        json!({"operation_id":"list-vehicles","variables":{"url":"http://attacker.example"}}),
    ] {
        let response = router
            .clone()
            .oneshot(
                request("POST", "/api/execute", json!({}))
                    .header(header::ORIGIN, ORIGIN)
                    .header(header::COOKIE, &cookie)
                    .header("x-app-capability", &capability)
                    .header("x-csrf-token", &csrf)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let text = payload(response).await.to_string();
        assert!(!text.contains("attacker.example"));
        assert!(!text.contains("another-account-vehicle"));
    }
}

#[tokio::test]
async fn oversized_bodies_and_duplicate_cookies_are_rejected() {
    let (state, token) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    let (cookie, csrf, capability) = login(&router, &token).await;
    let response = router
        .clone()
        .oneshot(
            request("POST", "/api/validate", json!({}))
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, &cookie)
                .header("x-app-capability", &capability)
                .header("x-csrf-token", &csrf)
                .body(Body::from(" ".repeat(33 * 1024)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let response = router
        .oneshot(
            request("GET", "/api/session", json!({}))
                .header(header::COOKIE, format!("{cookie}; {cookie}"))
                .header("x-app-capability", &capability)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn static_files_only_support_safe_methods() {
    let (state, _) = AppState::new_demo(8765).unwrap();
    let response = app(state)
        .oneshot(
            request("POST", "/", json!({}))
                .header(header::ORIGIN, ORIGIN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn traversal_and_encoded_aliases_cannot_expose_source_files() {
    let (state, _) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    for path in [
        "/../Cargo.toml",
        "/../../Cargo.toml",
        "/%2e%2e/Cargo.toml",
        "/..%5cCargo.toml",
        "/.env",
    ] {
        let response = router
            .clone()
            .oneshot(request("GET", path, json!({})).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn request_admitted_before_logout_cannot_execute_after_body_arrives() {
    let (state, token) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    let (cookie, csrf, capability) = login(&router, &token).await;
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (finish_tx, finish_rx) = tokio::sync::oneshot::channel();
    let stream = futures_util::stream::once(async move {
        started_tx.send(()).unwrap();
        finish_rx.await.unwrap();
        Ok::<_, std::io::Error>(axum::body::Bytes::from_static(
            br#"{"operation_id":"list-vehicles","variables":{}}"#,
        ))
    });
    let delayed = request("POST", "/api/execute", json!({}))
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, &cookie)
        .header("x-app-capability", &capability)
        .header("x-csrf-token", &csrf)
        .body(Body::from_stream(stream))
        .unwrap();
    let pending = tokio::spawn(router.clone().oneshot(delayed));
    started_rx.await.unwrap();
    let response = router
        .oneshot(
            request("POST", "/api/logout", json!({}))
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, &cookie)
                .header("x-app-capability", &capability)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    finish_tx.send(()).unwrap();
    assert_eq!(
        pending.await.unwrap().unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn host_scoped_cookie_alone_cannot_recover_or_use_a_local_session() {
    let (state, token) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    let (cookie, csrf, capability) = login(&router, &token).await;
    for path in ["/api/session", "/api/catalog", "/api/vehicles"] {
        for provided in [None, Some("incorrect-capability")] {
            let mut builder = request("GET", path, json!({})).header(header::COOKIE, &cookie);
            if let Some(provided) = provided {
                builder = builder.header("x-app-capability", provided);
            }
            let response = router
                .clone()
                .oneshot(builder.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
            let body = payload(response).await.to_string();
            assert!(!body.contains(&capability));
            assert!(!body.contains(&csrf));
        }
    }
    let response = router
        .clone()
        .oneshot(
            request("POST", "/api/execute", json!({}))
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({"operation_id":"list-vehicles","variables":{}}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    // A failed capability check must not revoke the owner's valid tab session.
    let response = router
        .oneshot(
            request("GET", "/api/session", json!({}))
                .header(header::COOKIE, &cookie)
                .header("x-app-capability", &capability)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = payload(response).await;
    assert_eq!(body["csrf_token"], csrf);
    assert!(
        body.get("app_capability").is_none(),
        "Session recovery must not issue a new capability."
    );
}

#[tokio::test]
async fn duplicate_or_another_instance_capabilities_are_not_accepted() {
    let (state, token) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    let (cookie, _, capability) = login(&router, &token).await;
    let (other_state, other_token) = AppState::new_demo(8765).unwrap();
    let (_, _, other_capability) = login(&app(other_state), &other_token).await;
    assert_ne!(capability, other_capability);
    for builder in [
        request("GET", "/api/session", json!({}))
            .header("x-app-capability", &capability)
            .header("x-app-capability", &capability),
        request("GET", "/api/session", json!({}))
            .header("x-app-capability", format!("{capability}, {capability}")),
        request("GET", "/api/session", json!({})).header("x-app-capability", other_capability),
    ] {
        let response = router
            .clone()
            .oneshot(
                builder
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn demo_mode_never_accepts_account_credentials() {
    let (state, token) = AppState::new_demo(8765).unwrap();
    let router = app(state);
    let (cookie, csrf, capability) = login(&router, &token).await;
    for (method, path, body) in [
        ("GET", "/api/account", json!({})),
        (
            "POST",
            "/api/account/login",
            json!({"email":"test@example.invalid", "password":"TEST_ONLY_NEVER_A_REAL_PASSWORD"}),
        ),
        ("POST", "/api/account/otp", json!({"code":"123456"})),
        ("POST", "/api/account/logout", json!({})),
    ] {
        let response = router
            .clone()
            .oneshot(
                request(method, path, json!({}))
                    .header(header::ORIGIN, ORIGIN)
                    .header(header::COOKIE, &cookie)
                    .header("x-app-capability", &capability)
                    .header("x-csrf-token", &csrf)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        let body = payload(response).await;
        assert_eq!(body["error"]["code"], "demo_mode");
        assert!(!body.to_string().contains("TEST_ONLY_NEVER_A_REAL_PASSWORD"));
    }
}

#[tokio::test]
async fn live_startup_is_signed_out_and_invalid_login_inputs_do_not_dispatch() {
    let (state, token) = AppState::new(8765).unwrap();
    let router = app(state);
    let (cookie, csrf, capability) = login(&router, &token).await;
    let response = router
        .clone()
        .oneshot(
            request("GET", "/api/account", json!({}))
                .header(header::COOKIE, &cookie)
                .header("x-app-capability", &capability)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        payload(response).await,
        json!({"state":"signed_out", "channel":null})
    );
    for (path, body, expected_code) in [
        (
            "/api/account/login",
            json!({"email":"invalid-address","password":"TEST_ONLY_PASSWORD"}),
            "invalid_credentials",
        ),
        (
            "/api/account/login",
            json!({"email":"test@example.invalid","password":""}),
            "invalid_credentials",
        ),
        (
            "/api/account/login",
            json!({"email":"test@example.invalid","password":"x".repeat(1025)}),
            "invalid_credentials",
        ),
        (
            "/api/account/login",
            json!({"email":format!("{}@example.invalid", "x".repeat(255)),"password":"TEST_ONLY_PASSWORD"}),
            "invalid_credentials",
        ),
        ("/api/account/otp", json!({"code":"abc123"}), "invalid_otp"),
        ("/api/account/otp", json!({"code":"123"}), "invalid_otp"),
        (
            "/api/account/otp",
            json!({"code":"1".repeat(13)}),
            "invalid_otp",
        ),
    ] {
        let response = router
            .clone()
            .oneshot(
                request("POST", path, json!({}))
                    .header(header::ORIGIN, ORIGIN)
                    .header(header::COOKIE, &cookie)
                    .header("x-app-capability", &capability)
                    .header("x-csrf-token", &csrf)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        let body = payload(response).await;
        assert_eq!(body["error"]["code"], expected_code);
        assert!(!body.to_string().contains("TEST_ONLY_PASSWORD"));
    }
}

#[tokio::test]
async fn account_authentication_requires_the_local_capability_and_csrf() {
    let (state, token) = AppState::new(8765).unwrap();
    let router = app(state);
    let (cookie, _, capability) = login(&router, &token).await;
    // Deliberately invalid fields keep this test offline even if boundary checks regress.
    for path in [
        "/api/account/login",
        "/api/account/otp",
        "/api/account/logout",
    ] {
        for provide_capability in [false, true] {
            let mut builder = request("POST", path, json!({}))
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, &cookie);
            if provide_capability {
                builder = builder.header("x-app-capability", &capability);
            }
            let response = router
                .clone()
                .oneshot(builder.body(Body::from("{}")).unwrap())
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                if provide_capability {
                    StatusCode::FORBIDDEN
                } else {
                    StatusCode::UNAUTHORIZED
                },
                "{path}"
            );
        }
    }
}

#[tokio::test]
async fn credentials_body_released_after_logout_cannot_start_authentication() {
    let (state, token) = AppState::new(8765).unwrap();
    let router = app(state);
    let (cookie, csrf, capability) = login(&router, &token).await;
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (finish_tx, finish_rx) = tokio::sync::oneshot::channel();
    let stream = futures_util::stream::once(async move {
        started_tx.send(()).unwrap();
        finish_rx.await.unwrap();
        // Invalid fields guarantee this stays offline if the security boundary regresses.
        Ok::<_, std::io::Error>(axum::body::Bytes::from_static(
            br#"{"email":"not-an-email","password":""}"#,
        ))
    });
    let delayed = request("POST", "/api/account/login", json!({}))
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, &cookie)
        .header("x-app-capability", &capability)
        .header("x-csrf-token", &csrf)
        .body(Body::from_stream(stream))
        .unwrap();
    let pending = tokio::spawn(router.clone().oneshot(delayed));
    started_rx.await.unwrap();
    let response = router
        .oneshot(
            request("POST", "/api/logout", json!({}))
                .header(header::ORIGIN, ORIGIN)
                .header(header::COOKIE, &cookie)
                .header("x-app-capability", &capability)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    finish_tx.send(()).unwrap();
    assert_eq!(
        pending.await.unwrap().unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}
