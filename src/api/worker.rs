//! Runs API calls off the UI thread, mirroring the AuthWorker pattern: mpsc
//! request/event channels, main loop polls each tick. Also owns the
//! results.jsonl forensics log - every posted body (with its local hash) and
//! every response lands there, which is the debugging trail for 461 hash
//! mismatches (see PLAN.md).

use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use crate::api::{ApeClient, ApiError, PostResultData};
use crate::auth::Session;
use crate::logging;

pub enum ApiRequest {
    /// Submit a finished CompletedEvent under the given session.
    PostResult {
        session: Session,
        event: serde_json::Value,
    },
}

pub enum ApiEvent {
    ResultPosted {
        outcome: Result<PostResultData, ApiError>,
        /// The session travels back so a mid-flight token refresh propagates.
        session: Session,
    },
}

pub struct ApiWorker {
    tx: Sender<ApiRequest>,
    rx: Receiver<ApiEvent>,
    pub pending: bool,
}

impl ApiWorker {
    pub fn spawn(client: ApeClient, results_log: Option<PathBuf>) -> Self {
        let (req_tx, req_rx) = std::sync::mpsc::channel::<ApiRequest>();
        let (evt_tx, evt_rx) = std::sync::mpsc::channel::<ApiEvent>();
        std::thread::spawn(move || {
            while let Ok(request) = req_rx.recv() {
                let event = match request {
                    ApiRequest::PostResult { mut session, event } => {
                        log_post(&results_log, &event);
                        let outcome = client.post_result(&mut session, &event);
                        log_outcome(&results_log, &outcome);
                        ApiEvent::ResultPosted { outcome, session }
                    }
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

    pub fn submit(&mut self, request: ApiRequest) {
        self.pending = true;
        let _ = self.tx.send(request);
    }

    /// Non-blocking poll for a completed API operation.
    pub fn poll(&mut self) -> Option<ApiEvent> {
        match self.rx.try_recv() {
            Ok(event) => {
                self.pending = false;
                Some(event)
            }
            Err(_) => None,
        }
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn log_post(path: &Option<PathBuf>, event: &serde_json::Value) {
    append_jsonl(
        path,
        &serde_json::json!({
            "kind": "post",
            "ts": now_ms() as u64,
            "hash": event.get("hash").cloned().unwrap_or(serde_json::Value::Null),
            "body": { "result": event },
        }),
    );
}

fn log_outcome(path: &Option<PathBuf>, outcome: &Result<PostResultData, ApiError>) {
    let line = match outcome {
        Ok(data) => serde_json::json!({
            "kind": "response",
            "ts": now_ms() as u64,
            "isPb": data.is_pb,
            "xp": data.xp,
            "insertedId": data.inserted_id,
        }),
        Err(ApiError::Server {
            status, message, ..
        }) => serde_json::json!({
            "kind": "error",
            "ts": now_ms() as u64,
            "status": status,
            "message": message,
        }),
        Err(other) => serde_json::json!({
            "kind": "error",
            "ts": now_ms() as u64,
            "message": other.to_string(),
        }),
    };
    append_jsonl(path, &line);
}

fn append_jsonl(path: &Option<PathBuf>, line: &serde_json::Value) {
    let Some(path) = path else { return };
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(mut file) => {
            let _ = writeln!(file, "{line}");
        }
        Err(e) => logging::debug(format_args!("results.jsonl append failed: {e}")),
    }
}
