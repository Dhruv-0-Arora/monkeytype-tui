//! ApeClient: blocking HTTP client for the official monkeytype API
//! (https://api.monkeytype.com). Bearer tokens come from AuthManager, which
//! auto-refreshes near expiry; on a 401 the token is force-refreshed once and
//! the request retried.

pub mod error;
pub mod models;
pub mod worker;

use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::auth::{AuthManager, Session};
use crate::logging;
pub use error::ApiError;
pub use models::{Envelope, PostResultData};

pub const BASE_URL: &str = "https://api.monkeytype.com";

/// Where bearer tokens come from. Production always goes through AuthManager;
/// tests can inject a canned sequence to exercise the 401-retry path without
/// real Firebase calls.
enum TokenSource {
    Auth(Arc<AuthManager>),
    #[cfg(test)]
    Canned(std::sync::Mutex<std::collections::VecDeque<String>>),
}

impl TokenSource {
    fn bearer(&self, session: &mut Session) -> Result<String, crate::auth::AuthError> {
        match self {
            TokenSource::Auth(auth) => auth.bearer(session),
            #[cfg(test)]
            TokenSource::Canned(_) => Ok(session.id_token.clone()),
        }
    }

    fn force_refresh(&self, session: &mut Session) -> Result<(), crate::auth::AuthError> {
        match self {
            TokenSource::Auth(auth) => auth.force_refresh(session),
            #[cfg(test)]
            TokenSource::Canned(tokens) => {
                let next = tokens.lock().expect("token lock").pop_front();
                session.id_token = next.ok_or_else(|| {
                    crate::auth::AuthError::Firebase("no canned token left".into())
                })?;
                Ok(())
            }
        }
    }
}

pub struct ApeClient {
    http: reqwest::blocking::Client,
    base: String,
    auth: TokenSource,
}

impl ApeClient {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        Self::with_base(auth, BASE_URL.to_string())
    }

    /// Custom base URL for tests (wiremock).
    pub fn with_base(auth: Arc<AuthManager>, base: String) -> Self {
        Self::with_token_source(TokenSource::Auth(auth), base)
    }

    /// Refresh tokens come from a canned list instead of Firebase.
    #[cfg(test)]
    fn with_canned_refresh(base: String, refreshed: Vec<String>) -> Self {
        Self::with_token_source(
            TokenSource::Canned(std::sync::Mutex::new(refreshed.into())),
            base,
        )
    }

    fn with_token_source(auth: TokenSource, base: String) -> Self {
        let version = env!("CARGO_PKG_VERSION");
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "X-Client-Version",
            format!("monkeytype-tui_{version}").parse().expect("header"),
        );
        headers.insert("Accept", "application/json".parse().expect("header"));
        let http = reqwest::blocking::Client::builder()
            .user_agent(format!("monkeytype-tui/{version}"))
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client builds");
        Self { http, base, auth }
    }

    #[cfg(test)]
    fn version_header() -> String {
        format!("monkeytype-tui_{}", env!("CARGO_PKG_VERSION"))
    }

    pub fn get_user(&self, session: &mut Session) -> Result<serde_json::Value, ApiError> {
        self.request(session, reqwest::Method::GET, "/users", None)
    }

    pub fn get_config(&self, session: &mut Session) -> Result<serde_json::Value, ApiError> {
        self.request(session, reqwest::Method::GET, "/configs", None)
    }

    pub fn patch_config(
        &self,
        session: &mut Session,
        delta: &serde_json::Value,
    ) -> Result<(), ApiError> {
        // PATCH /configs returns null data; only the status matters.
        self.request_envelope::<serde_json::Value>(
            session,
            reqwest::Method::PATCH,
            "/configs",
            Some(delta),
        )?;
        Ok(())
    }

    /// Submit a CompletedEvent. Body shape: `{"result": <event>}`.
    pub fn post_result(
        &self,
        session: &mut Session,
        result: &serde_json::Value,
    ) -> Result<PostResultData, ApiError> {
        let body = serde_json::json!({ "result": result });
        self.request(session, reqwest::Method::POST, "/results", Some(&body))
    }

    pub fn get_last_result(&self, session: &mut Session) -> Result<serde_json::Value, ApiError> {
        self.request(session, reqwest::Method::GET, "/results/last", None)
    }

    fn request<T: DeserializeOwned>(
        &self,
        session: &mut Session,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<T, ApiError> {
        self.request_envelope(session, method, path, body)?
            .data
            .ok_or_else(|| ApiError::Decode("response had no data".into()))
    }

    fn request_envelope<T: DeserializeOwned>(
        &self,
        session: &mut Session,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<Envelope<T>, ApiError> {
        let token = self.auth.bearer(session)?;
        let response = self.send(&method, path, body, &token)?;

        // One forced refresh + retry on 401 (token revoked or clock skew).
        let response = if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            logging::debug(format_args!("api {method} {path} -> 401, refreshing token"));
            self.auth.force_refresh(session)?;
            self.send(&method, path, body, &session.id_token)?
        } else {
            response
        };

        let status = response.status();
        let text = response
            .text()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        logging::debug(format_args!("api {method} {path} -> {status}"));

        if !status.is_success() {
            let message = serde_json::from_str::<Envelope<serde_json::Value>>(&text)
                .map(|e| e.message)
                .unwrap_or_default();
            return Err(ApiError::server(status.as_u16(), message));
        }
        serde_json::from_str(&text).map_err(|e| ApiError::Decode(e.to_string()))
    }

    fn send(
        &self,
        method: &reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
        token: &str,
    ) -> Result<reqwest::blocking::Response, ApiError> {
        let mut req = self
            .http
            .request(method.clone(), format!("{}{path}", self.base))
            .header("Authorization", format!("Bearer {token}"));
        if let Some(body) = body {
            req = req.json(body);
        }
        req.send().map_err(|e| {
            logging::debug(format_args!("api {method} {path} network error: {e}"));
            ApiError::Network(e.to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn session(token: &str) -> Session {
        Session {
            uid: "uid123".into(),
            email: None,
            id_token: token.into(),
            refresh_token: "refresh".into(),
            id_token_expiry: Instant::now() + Duration::from_secs(3600),
        }
    }

    /// A 401 must trigger exactly one forced refresh and a retry carrying the
    /// refreshed token.
    #[test]
    fn retries_once_with_refreshed_token_on_401() {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let server = rt.block_on(MockServer::start());
        rt.block_on(
            Mock::given(method("GET"))
                .and(path("/users"))
                .and(header("Authorization", "Bearer stale"))
                .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                    "message": "Unauthorized",
                    "data": null,
                })))
                .expect(1)
                .mount(&server),
        );
        rt.block_on(
            Mock::given(method("GET"))
                .and(path("/users"))
                .and(header("Authorization", "Bearer fresh"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "message": "ok",
                    "data": { "name": "test" },
                })))
                .expect(1)
                .mount(&server),
        );

        let client = ApeClient::with_canned_refresh(server.uri(), vec!["fresh".into()]);
        let mut s = session("stale");
        let user = client.get_user(&mut s).expect("retry should succeed");
        assert_eq!(user["name"], "test");
        assert_eq!(s.id_token, "fresh", "session carries the refreshed token");
        rt.block_on(server.verify());
    }

    /// A second consecutive 401 must NOT loop: the retried response is final.
    #[test]
    fn does_not_retry_twice_on_repeated_401() {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let server = rt.block_on(MockServer::start());
        rt.block_on(
            Mock::given(method("GET"))
                .and(path("/users"))
                .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                    "message": "Unauthorized",
                    "data": null,
                })))
                .expect(2)
                .mount(&server),
        );

        let client = ApeClient::with_canned_refresh(server.uri(), vec!["fresh".into()]);
        let mut s = session("stale");
        let err = client.get_user(&mut s).expect_err("still 401");
        match err {
            ApiError::Server { status, .. } => assert_eq!(status, 401),
            other => panic!("expected server error, got {other:?}"),
        }
        rt.block_on(server.verify());
    }

    #[test]
    fn client_sends_version_headers() {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let server = rt.block_on(MockServer::start());
        rt.block_on(
            Mock::given(method("GET"))
                .and(path("/users"))
                .and(header("X-Client-Version", ApeClient::version_header()))
                .and(header(
                    "User-Agent",
                    format!("monkeytype-tui/{}", env!("CARGO_PKG_VERSION")),
                ))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "message": "ok",
                    "data": {},
                })))
                .expect(1)
                .mount(&server),
        );
        let client = ApeClient::with_canned_refresh(server.uri(), vec![]);
        let mut s = session("tok");
        client.get_user(&mut s).expect("headers matched");
        rt.block_on(server.verify());
    }
}
