//! API error mapping, including the monkeytype 46x result-rejection codes
//! (backend/src/api/controllers/result.ts).

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Non-2xx from the API; `friendly` is what the UI shows.
    #[error("{friendly}")]
    Server {
        status: u16,
        message: String,
        friendly: String,
    },
    #[error("network error: {0}")]
    Network(String),
    #[error("could not read response: {0}")]
    Decode(String),
    #[error(transparent)]
    Auth(#[from] crate::auth::AuthError),
}

impl ApiError {
    pub fn server(status: u16, message: String) -> Self {
        let friendly = match status {
            460 => "test too short - the server needs 15s+ (time) or 10+ words".to_string(),
            461 => "result hash rejected (object-hash mismatch) - request logged to results.jsonl"
                .to_string(),
            462 => "submitted too soon after the previous result - wait a moment".to_string(),
            463 => "result data rejected by the server".to_string(),
            464 => "high-speed results need key timing data - use a kitty-protocol terminal \
                    (kitty, WezTerm, Ghostty, foot)"
                .to_string(),
            465 => "the server flagged this result as bot-like".to_string(),
            466 => "duplicate result".to_string(),
            _ if message.is_empty() => format!("server returned HTTP {status}"),
            _ => message.clone(),
        };
        ApiError::Server {
            status,
            message,
            friendly,
        }
    }

    /// Whether resubmitting the same result can possibly succeed (mirrors the
    /// web's retry blocklist in test-logic.ts).
    pub fn is_retryable(&self) -> bool {
        match self {
            ApiError::Server { status, .. } => !matches!(status, 460 | 461 | 463 | 464 | 465 | 466),
            ApiError::Network(_) | ApiError::Decode(_) | ApiError::Auth(_) => true,
        }
    }
}
