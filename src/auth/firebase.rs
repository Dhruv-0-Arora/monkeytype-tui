//! Firebase Auth REST calls against the official monkeytype Firebase project.
//!
//! The web config is public - it ships in the deployed monkeytype.com bundle
//! (js/firebase-config-live.*.js). Re-extract it by fetching monkeytype.com,
//! finding the firebase-config-live chunk, and reading the `apiKey`/`authDomain`
//! values. The API key is HTTP-referrer restricted, so every request must send
//! `Referer: https://monkeytype.com`; without it Google returns 403
//! API_KEY_HTTP_REFERRER_BLOCKED.

use std::time::{Duration, Instant};

use serde::Deserialize;

use super::{AuthError, Session};

pub const API_KEY: &str = "AIzaSyB5m_AnO575kvWriahcF1SFIWp8Fj3gQno";
pub const AUTH_DOMAIN: &str = "auth.monkeytype.com";
pub const REFERER: &str = "https://monkeytype.com";

const IDENTITY_BASE: &str = "https://identitytoolkit.googleapis.com/v1";
const SECURETOKEN_BASE: &str = "https://securetoken.googleapis.com/v1";

/// Allow overriding the API key via env, in case Google rotates it.
pub fn api_key() -> String {
    std::env::var("MONKEYTYPE_TUI_FIREBASE_API_KEY").unwrap_or_else(|_| API_KEY.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignInResponse {
    local_id: String,
    #[serde(default)]
    email: String,
    id_token: String,
    refresh_token: String,
    expires_in: String,
}

impl SignInResponse {
    fn into_session(self) -> Session {
        let ttl = self.expires_in.parse::<u64>().unwrap_or(3600);
        Session {
            uid: self.local_id,
            email: if self.email.is_empty() {
                None
            } else {
                Some(self.email)
            },
            id_token: self.id_token,
            refresh_token: self.refresh_token,
            id_token_expiry: Instant::now() + Duration::from_secs(ttl),
        }
    }
}

#[derive(Debug, Deserialize)]
struct FirebaseError {
    error: FirebaseErrorBody,
}

#[derive(Debug, Deserialize)]
struct FirebaseErrorBody {
    message: String,
}

/// Turn a Firebase error `message` code into a friendly AuthError.
fn map_error(status: u16, body: &str) -> AuthError {
    let code = serde_json::from_str::<FirebaseError>(body)
        .map(|e| e.error.message)
        .unwrap_or_else(|_| format!("HTTP {status}"));
    let friendly = match code.split('_').next().unwrap_or(&code) {
        _ if code.starts_with("EMAIL_NOT_FOUND") => "no account with that email",
        _ if code.starts_with("INVALID_PASSWORD") => "wrong password",
        _ if code.starts_with("INVALID_LOGIN_CREDENTIALS") => "wrong email or password",
        _ if code.starts_with("USER_DISABLED") => "this account is disabled",
        _ if code.starts_with("TOO_MANY_ATTEMPTS") => "too many attempts - try again later",
        _ if code.starts_with("INVALID_EMAIL") => "invalid email address",
        _ => return AuthError::Firebase(code),
    };
    AuthError::Firebase(friendly.to_string())
}

fn post_json(
    client: &reqwest::blocking::Client,
    url: &str,
    body: &serde_json::Value,
) -> Result<String, AuthError> {
    let resp = client
        .post(url)
        .header("Referer", REFERER)
        .json(body)
        .send()
        .map_err(|e| AuthError::Network(e.to_string()))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| AuthError::Network(e.to_string()))?;
    if status.is_success() {
        Ok(text)
    } else {
        Err(map_error(status.as_u16(), &text))
    }
}

pub fn sign_in_password(
    client: &reqwest::blocking::Client,
    email: &str,
    password: &str,
) -> Result<Session, AuthError> {
    let url = format!(
        "{IDENTITY_BASE}/accounts:signInWithPassword?key={}",
        api_key()
    );
    let body = serde_json::json!({
        "email": email,
        "password": password,
        "returnSecureToken": true,
    });
    let text = post_json(client, &url, &body)?;
    let resp: SignInResponse =
        serde_json::from_str(&text).map_err(|e| AuthError::Decode(e.to_string()))?;
    Ok(resp.into_session())
}

/// Build the provider authorization URL and its sessionId (OAuth handler flow).
/// Firebase fills in the provider client_id and accepts our loopback redirect.
pub fn create_auth_uri(
    client: &reqwest::blocking::Client,
    provider_id: &str,
    continue_uri: &str,
) -> Result<(String, String), AuthError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct AuthUriResponse {
        auth_uri: String,
        session_id: String,
    }
    let url = format!("{IDENTITY_BASE}/accounts:createAuthUri?key={}", api_key());
    let body = serde_json::json!({
        "providerId": provider_id,
        "continueUri": continue_uri,
    });
    let text = post_json(client, &url, &body)?;
    let resp: AuthUriResponse =
        serde_json::from_str(&text).map_err(|e| AuthError::Decode(e.to_string()))?;
    Ok((resp.auth_uri, resp.session_id))
}

/// Complete the OAuth handler flow: hand Firebase the full provider redirect URL
/// plus the sessionId. Firebase exchanges an auth code server-side (it holds the
/// provider secret - this is why GitHub works without us having the secret) and
/// returns Firebase tokens.
pub fn sign_in_with_idp(
    client: &reqwest::blocking::Client,
    request_uri: &str,
    session_id: &str,
) -> Result<Session, AuthError> {
    let url = format!("{IDENTITY_BASE}/accounts:signInWithIdp?key={}", api_key());
    let body = serde_json::json!({
        "requestUri": request_uri,
        "sessionId": session_id,
        "returnSecureToken": true,
        "returnIdpCredential": true,
    });
    let text = post_json(client, &url, &body)?;
    let resp: SignInResponse =
        serde_json::from_str(&text).map_err(|e| AuthError::Decode(e.to_string()))?;
    Ok(resp.into_session())
}

/// Exchange a refresh token for a fresh id token.
pub fn refresh_token(
    client: &reqwest::blocking::Client,
    refresh_token: &str,
) -> Result<(String, String, u64), AuthError> {
    #[derive(Deserialize)]
    struct RefreshResponse {
        id_token: String,
        refresh_token: String,
        expires_in: String,
    }
    let url = format!("{SECURETOKEN_BASE}/token?key={}", api_key());
    let resp = client
        .post(&url)
        .header("Referer", REFERER)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .map_err(|e| AuthError::Network(e.to_string()))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| AuthError::Network(e.to_string()))?;
    if !status.is_success() {
        return Err(map_error(status.as_u16(), &text));
    }
    let r: RefreshResponse =
        serde_json::from_str(&text).map_err(|e| AuthError::Decode(e.to_string()))?;
    let ttl = r.expires_in.parse::<u64>().unwrap_or(3600);
    Ok((r.id_token, r.refresh_token, ttl))
}
