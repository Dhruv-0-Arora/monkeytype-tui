//! Opt-in debug logging for interactive forensics. Raw terminal mode makes
//! stdout/stderr unusable, so diagnostics go to `{data_dir}/debug.log` instead,
//! and only when `MONKEYTYPE_TUI_DEBUG=1` is set. Never log secrets: no tokens,
//! passwords, API keys, or full URLs with query strings.

use std::fmt::Display;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_FILE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Wire up the log destination once at startup. A `None` data dir or an unset
/// `MONKEYTYPE_TUI_DEBUG` turns every later `debug()` into a no-op.
pub fn init(data_dir: Option<&Path>) {
    let enabled = std::env::var("MONKEYTYPE_TUI_DEBUG").is_ok_and(|v| v == "1");
    let path = match (enabled, data_dir) {
        (true, Some(dir)) => Some(dir.join("debug.log")),
        _ => None,
    };
    let _ = LOG_FILE.set(path);
}

/// Append one timestamped line. Safe to call from any thread; silently drops
/// the line if logging is disabled or the file cannot be written.
pub fn debug(msg: impl Display) {
    let Some(Some(path)) = LOG_FILE.get() else {
        return;
    };
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let line = format!("[{}.{:03}] {}\n", ts.as_secs(), ts.subsec_millis(), msg);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

/// A URL with its query string (which may carry keys/credentials) removed.
pub fn redact_url(url: &str) -> &str {
    url.split(['?', '#']).next().unwrap_or(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_query_and_fragment() {
        assert_eq!(
            redact_url("https://x.test/v1/token?key=SECRET"),
            "https://x.test/v1/token"
        );
        assert_eq!(
            redact_url("https://x.test/cb#id_token=SECRET"),
            "https://x.test/cb"
        );
        assert_eq!(redact_url("https://x.test/plain"), "https://x.test/plain");
    }
}
