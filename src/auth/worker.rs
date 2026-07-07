//! Runs auth operations off the UI thread so the sync render loop stays
//! responsive. Every request here is a short network call; the long human part
//! of browser OAuth (signing in, copying the redirect URL) happens in the UI,
//! so one slow flow can never queue-block a later login attempt.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use crate::logging;

use super::{AuthManager, OAuthProvider, Session};

pub enum AuthRequest {
    /// Restore a prior session from the stored refresh token (startup).
    Restore,
    Email {
        email: String,
        password: String,
    },
    /// Start browser OAuth: build the provider URL and open the browser.
    OAuthBegin(OAuthProvider),
    /// Finish browser OAuth with the redirect URL pasted by the user.
    OAuthFinish(String),
    /// Abandon an in-progress OAuth flow (user pressed escape).
    OAuthCancel,
}

pub enum AuthEvent {
    LoggedIn(Session),
    /// Restore found no stored session (or it was stale) - stay logged out.
    NotRestored,
    Failed(String),
    /// OAuth started: show `auth_uri` and prompt for the pasted redirect URL.
    OAuthPending {
        provider: OAuthProvider,
        auth_uri: String,
        open_failed: bool,
    },
    /// An OAuth flow was cancelled; nothing to display.
    Cancelled,
}

pub struct AuthWorker {
    tx: Sender<AuthRequest>,
    rx: Receiver<AuthEvent>,
    pub pending: bool,
}

impl AuthWorker {
    pub fn spawn(manager: Arc<AuthManager>) -> Self {
        let (req_tx, req_rx) = std::sync::mpsc::channel::<AuthRequest>();
        let (evt_tx, evt_rx) = std::sync::mpsc::channel::<AuthEvent>();
        std::thread::spawn(move || {
            // sessionId of the OAuth flow awaiting its pasted redirect URL
            let mut oauth_flow: Option<(OAuthProvider, String)> = None;
            while let Ok(request) = req_rx.recv() {
                let event = match request {
                    AuthRequest::Restore => {
                        logging::debug("auth: restore requested");
                        match manager.restore() {
                            Some(session) => AuthEvent::LoggedIn(session),
                            None => AuthEvent::NotRestored,
                        }
                    }
                    AuthRequest::Email { email, password } => {
                        logging::debug("auth: email login requested");
                        match manager.login_email(&email, &password) {
                            Ok(session) => AuthEvent::LoggedIn(session),
                            Err(e) => AuthEvent::Failed(e.to_string()),
                        }
                    }
                    AuthRequest::OAuthBegin(provider) => {
                        logging::debug(format_args!("auth: oauth begin ({})", provider.label()));
                        match manager.oauth_begin(provider) {
                            Ok((auth_uri, session_id)) => {
                                oauth_flow = Some((provider, session_id));
                                let open_failed = match open::that(&auth_uri) {
                                    Ok(()) => {
                                        logging::debug("auth: browser opened");
                                        false
                                    }
                                    Err(e) => {
                                        logging::debug(format_args!(
                                            "auth: browser open failed: {e}"
                                        ));
                                        true
                                    }
                                };
                                AuthEvent::OAuthPending {
                                    provider,
                                    auth_uri,
                                    open_failed,
                                }
                            }
                            Err(e) => AuthEvent::Failed(e.to_string()),
                        }
                    }
                    AuthRequest::OAuthFinish(url) => {
                        logging::debug(format_args!(
                            "auth: oauth finish with pasted url {}",
                            logging::redact_url(&url)
                        ));
                        match oauth_flow.take() {
                            Some((provider, session_id)) => {
                                match manager.oauth_finish(provider, &url, &session_id) {
                                    Ok(session) => AuthEvent::LoggedIn(session),
                                    Err(e) => AuthEvent::Failed(e.to_string()),
                                }
                            }
                            None => AuthEvent::Failed("no sign-in in progress".into()),
                        }
                    }
                    AuthRequest::OAuthCancel => {
                        logging::debug("auth: oauth cancelled");
                        oauth_flow = None;
                        AuthEvent::Cancelled
                    }
                };
                match &event {
                    AuthEvent::LoggedIn(_) => logging::debug("auth: logged in"),
                    AuthEvent::NotRestored => logging::debug("auth: nothing to restore"),
                    AuthEvent::Failed(msg) => logging::debug(format_args!("auth: failed: {msg}")),
                    AuthEvent::OAuthPending { .. } | AuthEvent::Cancelled => {}
                }
                if evt_tx.send(event).is_err() {
                    break;
                }
            }
        });
        Self {
            tx: req_tx,
            rx: evt_rx,
            pending: false,
        }
    }

    pub fn submit(&mut self, request: AuthRequest) {
        self.pending = true;
        let _ = self.tx.send(request);
    }

    /// Non-blocking poll for a completed auth operation.
    pub fn poll(&mut self) -> Option<AuthEvent> {
        match self.rx.try_recv() {
            Ok(event) => {
                self.pending = false;
                Some(event)
            }
            Err(_) => None,
        }
    }
}
