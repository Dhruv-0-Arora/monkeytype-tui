//! Browser OAuth via Firebase's hosted handler page (createAuthUri ->
//! browser -> paste redirect URL -> signInWithIdp). We never need a provider
//! client secret: Firebase holds it and exchanges GitHub's code itself.
//!
//! Why no localhost loopback: monkeytype's Google and GitHub OAuth apps only
//! register `https://auth.monkeytype.com/__/auth/handler` as a redirect URI
//! (verified live - both providers reject a 127.0.0.1 redirect with
//! redirect_uri_mismatch). So the browser necessarily lands on the handler
//! page, which we cannot observe from this process. The user copies the final
//! URL from the address bar and pastes it into the TUI instead.
//!
//! Provider response shapes at the handler:
//! - GitHub uses `response_type=code`: the credential is in the query
//!   (`?code=...&state=...`), passed to signInWithIdp as `requestUri`.
//! - Google uses `response_type=id_token`: the credential is in the fragment
//!   (`#id_token=...`), which signInWithIdp cannot read from a URL, so it is
//!   converted into the `postBody` form.

use super::firebase;
use super::{AuthError, OAuthProvider, Session};

/// Start the flow: build the provider authorization URL (redirecting to the
/// Firebase handler) and the sessionId that `finish` needs.
pub fn begin(
    client: &reqwest::blocking::Client,
    provider: OAuthProvider,
) -> Result<(String, String), AuthError> {
    let continue_uri = format!("https://{}/__/auth/handler", firebase::AUTH_DOMAIN);
    firebase::create_auth_uri(client, provider.provider_id(), &continue_uri)
}

/// Complete the flow with the URL the user copied from the browser address bar
/// after signing in at the provider.
pub fn finish(
    client: &reqwest::blocking::Client,
    provider: OAuthProvider,
    pasted_url: &str,
    session_id: &str,
) -> Result<Session, AuthError> {
    let url = pasted_url.trim();
    if url.is_empty() {
        return Err(AuthError::Loopback("no redirect URL pasted".into()));
    }
    if let Some(denial) = denial_message(url) {
        return Err(AuthError::Loopback(denial));
    }

    // Fragment credentials (Google id_token flow) must travel via postBody;
    // query credentials (GitHub code flow) go through requestUri untouched.
    if let Some(fragment) = url.split('#').nth(1) {
        if fragment.contains("id_token=") || fragment.contains("access_token=") {
            let post_body = format!("{fragment}&providerId={}", provider.provider_id());
            let base = url.split('#').next().unwrap_or(url);
            return firebase::sign_in_with_idp(client, base, Some(&post_body), session_id);
        }
    }
    firebase::sign_in_with_idp(client, url, None, session_id)
}

/// If the provider redirected back with an OAuth error, return its actual
/// message instead of a generic "denied".
fn denial_message(url: &str) -> Option<String> {
    let params = url
        .split_once(['?', '#'])
        .map(|(_, rest)| rest)?
        .split(['&', '#']);
    let mut error = None;
    let mut description = None;
    for param in params {
        if let Some(v) = param.strip_prefix("error=") {
            // "error_description=" and "error_uri=" must not match "error="
            error = Some(v);
        } else if let Some(v) = param.strip_prefix("error_description=") {
            description = Some(v);
        }
    }
    let error = error?;
    let text = match description {
        Some(desc) => format!("{error}: {}", percent_decode(desc)),
        None => percent_decode(error),
    };
    Some(format!("provider returned an error - {text}"))
}

/// Minimal percent-decoding for OAuth error descriptions ('+' as space).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl OAuthProvider {
    pub fn provider_id(self) -> &'static str {
        match self {
            OAuthProvider::Google => "google.com",
            OAuthProvider::Github => "github.com",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denial_extracts_provider_error() {
        let url = "https://auth.monkeytype.com/__/auth/handler?error=access_denied&error_description=The+user+has+denied+access&state=x";
        let msg = denial_message(url).expect("detects error param");
        assert!(msg.contains("access_denied"), "{msg}");
        assert!(msg.contains("The user has denied access"), "{msg}");
    }

    #[test]
    fn denial_ignores_success_redirects() {
        assert_eq!(
            denial_message("https://auth.monkeytype.com/__/auth/handler?code=abc&state=x"),
            None
        );
        assert_eq!(
            denial_message("https://auth.monkeytype.com/__/auth/handler#id_token=abc"),
            None
        );
        // error_description alone (no error=) is not a denial marker
        assert_eq!(
            denial_message("https://x.test/cb?error_description=only"),
            None
        );
    }

    #[test]
    fn percent_decode_handles_escapes() {
        assert_eq!(percent_decode("a%20b+c%2Fd"), "a b c/d");
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("bad%zz"), "bad%zz");
    }
}
