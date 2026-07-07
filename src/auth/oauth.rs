//! Browser-based OAuth via a localhost loopback, using Firebase's OAuth handler
//! flow (createAuthUri -> browser -> signInWithIdp). We never need a provider
//! client secret: for the GitHub code flow Firebase exchanges the code itself.
//!
//! Two provider response shapes are handled at the callback:
//! - GitHub uses `response_type=code`, so the code arrives as a query param and
//!   the loopback captures it directly.
//! - Google uses `response_type=id_token`, so the credential lands in the URL
//!   fragment; a tiny HTML+JS page copies the fragment into a second request the
//!   loopback can read.

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::time::Duration;

use super::firebase;
use super::{AuthError, OAuthProvider, Session};

/// How long to wait for the user to finish in the browser.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(180);

pub fn login(
    client: &reqwest::blocking::Client,
    provider: OAuthProvider,
) -> Result<Session, AuthError> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|e| AuthError::Loopback(e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| AuthError::Loopback(e.to_string()))?
        .port();
    let continue_uri = format!("http://127.0.0.1:{port}/callback");

    let (auth_uri, session_id) =
        firebase::create_auth_uri(client, provider.provider_id(), &continue_uri)?;

    // Best-effort browser launch; the URL is also surfaced for copy-paste.
    let _ = open::that(&auth_uri);

    let request_uri = wait_for_redirect(&listener, port)?;
    firebase::sign_in_with_idp(client, &request_uri, &session_id)
}

/// The authorization URL for the given provider (also shown in the TUI so the
/// user can paste it into a browser on a headless/remote machine).
pub fn auth_url(
    client: &reqwest::blocking::Client,
    provider: OAuthProvider,
    port: u16,
) -> Result<(String, String), AuthError> {
    let continue_uri = format!("http://127.0.0.1:{port}/callback");
    firebase::create_auth_uri(client, provider.provider_id(), &continue_uri)
}

/// Block until the browser hits our loopback, returning the full redirect URL
/// (with code/token) to hand to signInWithIdp.
fn wait_for_redirect(listener: &TcpListener, port: u16) -> Result<String, AuthError> {
    listener
        .set_nonblocking(false)
        .map_err(|e| AuthError::Loopback(e.to_string()))?;
    let deadline = std::time::Instant::now() + LOGIN_TIMEOUT;

    // Accept connections until one carries the credential. The fragment case
    // (Google) needs two hits: first the JS page, then the copied fragment.
    loop {
        if std::time::Instant::now() > deadline {
            return Err(AuthError::Loopback("timed out waiting for browser".into()));
        }
        listener
            .set_nonblocking(true)
            .map_err(|e| AuthError::Loopback(e.to_string()))?;
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(uri) = handle_connection(stream, port)? {
                    return Ok(uri);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(AuthError::Loopback(e.to_string())),
        }
    }
}

/// Parse one HTTP request line. Returns Some(full_callback_url) once we have the
/// credential, or None if this was an intermediate request (favicon, the JS
/// bootstrap page for the fragment flow, etc.).
fn handle_connection(mut stream: TcpStream, port: u16) -> Result<Option<String>, AuthError> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|e| AuthError::Loopback(e.to_string()))?,
    );
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| AuthError::Loopback(e.to_string()))?;

    // "GET /callback?code=...&state=... HTTP/1.1"
    let path = request_line.split_whitespace().nth(1).unwrap_or("/");

    // The fragment-relay page posts back to /token?<fragment>.
    if let Some(query) = path.strip_prefix("/token?") {
        respond(&mut stream, DONE_PAGE);
        let full = format!("http://127.0.0.1:{port}/callback?{query}");
        return Ok(Some(full));
    }

    if let Some(rest) = path.strip_prefix("/callback") {
        // Query present -> code flow (GitHub): capture directly.
        if let Some(query) = rest.strip_prefix('?') {
            if query.contains("error") {
                respond(&mut stream, DONE_PAGE);
                return Err(AuthError::Loopback("authorization was denied".into()));
            }
            respond(&mut stream, DONE_PAGE);
            let full = format!("http://127.0.0.1:{port}/callback?{query}");
            return Ok(Some(full));
        }
        // No query -> the credential is in the fragment (Google id_token flow).
        // Serve JS that relays the fragment back to /token.
        respond(&mut stream, FRAGMENT_RELAY_PAGE);
        return Ok(None);
    }

    // Anything else (e.g. /favicon.ico): ignore.
    respond(&mut stream, DONE_PAGE);
    Ok(None)
}

fn respond(stream: &mut TcpStream, body: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Write);
}

const DONE_PAGE: &str = "<!doctype html><html><body style=\"font-family:sans-serif;background:#323437;color:#d1d0c5;text-align:center;padding-top:4rem\"><h2>monkeytype-tui</h2><p>Login complete. You can close this tab and return to the terminal.</p></body></html>";

// Copies the URL fragment (#id_token=...) into a request the loopback can read,
// since fragments are never sent to the server directly.
const FRAGMENT_RELAY_PAGE: &str = "<!doctype html><html><body style=\"font-family:sans-serif;background:#323437;color:#d1d0c5;text-align:center;padding-top:4rem\"><h2>monkeytype-tui</h2><p>Finishing login...</p><script>var f=window.location.hash.substring(1);window.location.replace('/token?'+f);</script></body></html>";

impl OAuthProvider {
    pub fn provider_id(self) -> &'static str {
        match self {
            OAuthProvider::Google => "google.com",
            OAuthProvider::Github => "github.com",
        }
    }
}
