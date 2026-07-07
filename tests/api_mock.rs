//! ApeClient contract tests against a wiremock server: exact request shapes
//! (method, path, headers, body) and the 46x error mapping. The blocking
//! client is called from the test thread, outside the tokio runtime that
//! hosts the mock server, so there is no runtime conflict.

use std::sync::Arc;
use std::time::{Duration, Instant};

use monkeytype_tui::api::{ApeClient, ApiError};
use monkeytype_tui::auth::{AuthManager, Session, TokenStore};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(base: String) -> ApeClient {
    let tmp = std::env::temp_dir().join(format!("mttui-apitest-{}.json", std::process::id()));
    let store = TokenStore::with_account(tmp, format!("test-api-{}", std::process::id()));
    ApeClient::with_base(Arc::new(AuthManager::new(store)), base)
}

/// A session whose token is nowhere near expiry, so bearer() never touches
/// Firebase or the keychain.
fn far_session() -> Session {
    Session {
        uid: "uid123".into(),
        email: Some("test@example.com".into()),
        id_token: "tok123".into(),
        refresh_token: "refresh123".into(),
        id_token_expiry: Instant::now() + Duration::from_secs(3600),
    }
}

fn sample_result() -> serde_json::Value {
    serde_json::json!({
        "wpm": 60.5,
        "rawWpm": 62,
        "acc": 98.11,
        "mode": "words",
        "mode2": "25",
        "uid": "uid123",
        "hash": "abc123",
    })
}

#[test]
fn post_result_sends_exact_body_and_headers() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let server = rt.block_on(MockServer::start());
    rt.block_on(
        Mock::given(method("POST"))
            .and(path("/results"))
            .and(header("Authorization", "Bearer tok123"))
            .and(header("Content-Type", "application/json"))
            .and(header(
                "X-Client-Version",
                format!("monkeytype-tui_{}", env!("CARGO_PKG_VERSION")),
            ))
            .and(body_json(serde_json::json!({ "result": sample_result() })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": "Result saved",
                "data": {
                    "isPb": true,
                    "xp": 120,
                    "dailyXpBonus": false,
                    "streak": 4,
                    "insertedId": "65f000000000000000000000",
                    "xpBreakdown": { "base": 100 },
                    // unknown fields must be tolerated:
                    "someFutureField": { "nested": true },
                },
            })))
            .expect(1)
            .mount(&server),
    );

    let data = client(server.uri())
        .post_result(&mut far_session(), &sample_result())
        .expect("post succeeds");
    assert!(data.is_pb);
    assert_eq!(data.xp, 120);
    assert_eq!(data.streak, 4);
    assert_eq!(data.inserted_id, "65f000000000000000000000");
    rt.block_on(server.verify());
}

#[test]
fn result_rejection_codes_map_to_friendly_messages() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let cases: &[(u16, &str, bool)] = &[
        (460, "too short", false),
        (461, "hash", false),
        (462, "too soon", true),
        (463, "rejected", false),
        (464, "kitty", false),
        (465, "bot", false),
        (466, "duplicate", false),
    ];
    for &(status, expect_fragment, retryable) in cases {
        let server = rt.block_on(MockServer::start());
        rt.block_on(
            Mock::given(method("POST"))
                .and(path("/results"))
                .respond_with(
                    ResponseTemplate::new(status).set_body_json(serde_json::json!({
                        "message": "server says no",
                        "data": null,
                    })),
                )
                .mount(&server),
        );
        let err = client(server.uri())
            .post_result(&mut far_session(), &sample_result())
            .expect_err("must fail");
        match &err {
            ApiError::Server {
                status: got,
                friendly,
                message,
            } => {
                assert_eq!(*got, status);
                assert_eq!(message, "server says no");
                assert!(
                    friendly.to_lowercase().contains(expect_fragment),
                    "{status}: friendly message {friendly:?} should mention {expect_fragment:?}"
                );
            }
            other => panic!("expected server error, got {other:?}"),
        }
        assert_eq!(err.is_retryable(), retryable, "retryable for {status}");
    }
}

#[test]
fn server_errors_use_message_verbatim_and_are_retryable() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let server = rt.block_on(MockServer::start());
    rt.block_on(
        Mock::given(method("GET"))
            .and(path("/users"))
            .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({
                "message": "down for maintenance",
                "data": null,
            })))
            .mount(&server),
    );
    let err = client(server.uri())
        .get_user(&mut far_session())
        .expect_err("must fail");
    assert_eq!(err.to_string(), "down for maintenance");
    assert!(err.is_retryable());
}

#[test]
fn get_last_result_unwraps_envelope() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let server = rt.block_on(MockServer::start());
    rt.block_on(
        Mock::given(method("GET"))
            .and(path("/results/last"))
            .and(header("Authorization", "Bearer tok123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": "Result retrieved",
                "data": { "wpm": 60.5, "mode": "words" },
            })))
            .mount(&server),
    );
    let last = client(server.uri())
        .get_last_result(&mut far_session())
        .expect("fetch succeeds");
    assert_eq!(last["wpm"], 60.5);
    assert_eq!(last["mode"], "words");
}

#[test]
fn patch_config_sends_delta_and_accepts_null_data() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let server = rt.block_on(MockServer::start());
    let delta = serde_json::json!({ "blindMode": true, "theme": "serika_dark" });
    rt.block_on(
        Mock::given(method("PATCH"))
            .and(path("/configs"))
            .and(body_json(delta.clone()))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": "Config updated",
                "data": null,
            })))
            .expect(1)
            .mount(&server),
    );
    client(server.uri())
        .patch_config(&mut far_session(), &delta)
        .expect("patch succeeds");
    rt.block_on(server.verify());
}
