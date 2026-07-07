//! Runs auth operations off the UI thread. Login (especially browser OAuth) can
//! take seconds; the sync render loop stays responsive by dispatching requests
//! to a worker thread and polling results each tick.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use super::{AuthManager, OAuthProvider, Session};

pub enum AuthRequest {
    /// Restore a prior session from the stored refresh token (startup).
    Restore,
    Email {
        email: String,
        password: String,
    },
    OAuth(OAuthProvider),
}

pub enum AuthEvent {
    LoggedIn(Session),
    /// Restore found no stored session (or it was stale) - stay logged out.
    NotRestored,
    Failed(String),
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
            while let Ok(request) = req_rx.recv() {
                let event = match request {
                    AuthRequest::Restore => match manager.restore() {
                        Some(session) => AuthEvent::LoggedIn(session),
                        None => AuthEvent::NotRestored,
                    },
                    AuthRequest::Email { email, password } => {
                        match manager.login_email(&email, &password) {
                            Ok(session) => AuthEvent::LoggedIn(session),
                            Err(e) => AuthEvent::Failed(e.to_string()),
                        }
                    }
                    AuthRequest::OAuth(provider) => match manager.login_oauth(provider) {
                        Ok(session) => AuthEvent::LoggedIn(session),
                        Err(e) => AuthEvent::Failed(e.to_string()),
                    },
                };
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
