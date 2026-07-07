//! Authentication: Firebase email/password and browser OAuth (Google, GitHub),
//! plus persistent token storage and auto-refresh. See PLAN.md.

pub mod firebase;
pub mod oauth;
mod store;
pub mod worker;

use std::time::{Duration, Instant};

pub use store::TokenStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthProvider {
    Google,
    Github,
}

impl OAuthProvider {
    pub fn label(self) -> &'static str {
        match self {
            OAuthProvider::Google => "Google",
            OAuthProvider::Github => "GitHub",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("{0}")]
    Firebase(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("could not read response: {0}")]
    Decode(String),
    #[error("browser login failed: {0}")]
    Loopback(String),
    #[error("token storage error: {0}")]
    Storage(String),
}

/// An authenticated session. Only the refresh token is persisted to disk; the
/// short-lived id token stays in memory.
#[derive(Debug, Clone)]
pub struct Session {
    pub uid: String,
    pub email: Option<String>,
    pub id_token: String,
    pub refresh_token: String,
    pub id_token_expiry: Instant,
}

impl Session {
    /// Refresh a bit early so requests never race the 1h expiry.
    fn needs_refresh(&self) -> bool {
        self.id_token_expiry
            .checked_duration_since(Instant::now())
            .map(|left| left < Duration::from_secs(5 * 60))
            .unwrap_or(true)
    }
}

pub struct AuthManager {
    http: reqwest::blocking::Client,
    store: TokenStore,
}

impl AuthManager {
    pub fn new(store: TokenStore) -> Self {
        let http = reqwest::blocking::Client::builder()
            .user_agent(concat!("monkeytype-tui/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client builds");
        Self { http, store }
    }

    pub fn login_email(&self, email: &str, password: &str) -> Result<Session, AuthError> {
        let session = firebase::sign_in_password(&self.http, email, password)?;
        self.store.save(&session.refresh_token)?;
        Ok(session)
    }

    /// Start browser OAuth: returns (authorization URL, sessionId).
    pub fn oauth_begin(&self, provider: OAuthProvider) -> Result<(String, String), AuthError> {
        oauth::begin(&self.http, provider)
    }

    /// Finish browser OAuth with the redirect URL the user pasted.
    pub fn oauth_finish(
        &self,
        provider: OAuthProvider,
        pasted_url: &str,
        session_id: &str,
    ) -> Result<Session, AuthError> {
        let session = oauth::finish(&self.http, provider, pasted_url, session_id)?;
        self.store.save(&session.refresh_token)?;
        Ok(session)
    }

    /// Restore a prior session from the stored refresh token (startup path).
    pub fn restore(&self) -> Option<Session> {
        let refresh = self.store.load().ok().flatten()?;
        match firebase::refresh_token(&self.http, &refresh) {
            Ok((id_token, new_refresh, ttl)) => {
                let _ = self.store.save(&new_refresh);
                Some(Session {
                    uid: uid_from_jwt(&id_token).unwrap_or_default(),
                    email: None,
                    id_token,
                    refresh_token: new_refresh,
                    id_token_expiry: Instant::now() + Duration::from_secs(ttl),
                })
            }
            // stored token is stale/revoked - drop it
            Err(_) => {
                let _ = self.store.clear();
                None
            }
        }
    }

    /// Return a valid bearer token, refreshing in place if it is close to expiry.
    pub fn bearer(&self, session: &mut Session) -> Result<String, AuthError> {
        if session.needs_refresh() {
            self.force_refresh(session)?;
        }
        Ok(session.id_token.clone())
    }

    /// Unconditionally exchange the refresh token for a fresh id token (e.g.
    /// after the API rejected the current one with a 401).
    pub fn force_refresh(&self, session: &mut Session) -> Result<(), AuthError> {
        let (id_token, new_refresh, ttl) =
            firebase::refresh_token(&self.http, &session.refresh_token)?;
        session.id_token = id_token;
        session.refresh_token = new_refresh.clone();
        session.id_token_expiry = Instant::now() + Duration::from_secs(ttl);
        let _ = self.store.save(&new_refresh);
        Ok(())
    }

    pub fn logout(&self) {
        let _ = self.store.clear();
    }
}

/// Extract the `user_id`/`sub` claim from a Firebase JWT without verifying it
/// (verification is Google's job; we only need the uid for display/restore).
fn uid_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64_url_decode(payload)?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    json.get("user_id")
        .or_else(|| json.get("sub"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn base64_url_decode(input: &str) -> Option<Vec<u8>> {
    // minimal base64url decoder (no padding), avoids a dependency
    const fn val(c: u8) -> i16 {
        match c {
            b'A'..=b'Z' => (c - b'A') as i16,
            b'a'..=b'z' => (c - b'a' + 26) as i16,
            b'0'..=b'9' => (c - b'0' + 52) as i16,
            b'-' => 62,
            b'_' => 63,
            _ => -1,
        }
    }
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &c in input.as_bytes() {
        let v = val(c);
        if v < 0 {
            continue;
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_jwt_uid() {
        // {"user_id":"abc123","sub":"abc123"} base64url, no signature needed
        let payload = "eyJ1c2VyX2lkIjoiYWJjMTIzIiwic3ViIjoiYWJjMTIzIn0";
        let token = format!("header.{payload}.sig");
        assert_eq!(uid_from_jwt(&token), Some("abc123".to_string()));
    }

    #[test]
    fn base64url_decodes_known_value() {
        assert_eq!(base64_url_decode("aGVsbG8"), Some(b"hello".to_vec()));
    }
}
