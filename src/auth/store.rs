//! Refresh-token persistence: OS keychain first, a 0600 file as fallback.

use std::path::PathBuf;

use super::AuthError;

const SERVICE: &str = "monkeytype-tui";
const ACCOUNT: &str = "refresh-token";

pub struct TokenStore {
    fallback_path: PathBuf,
    /// Keychain account name; overridable so tests don't touch the real slot.
    account: String,
}

impl TokenStore {
    pub fn new(fallback_path: PathBuf) -> Self {
        Self {
            fallback_path,
            account: ACCOUNT.to_string(),
        }
    }

    /// Isolated store using a distinct keychain account (test-only).
    pub fn with_account(fallback_path: PathBuf, account: impl Into<String>) -> Self {
        Self {
            fallback_path,
            account: account.into(),
        }
    }

    pub fn save(&self, refresh_token: &str) -> Result<(), AuthError> {
        if let Ok(entry) = keyring::Entry::new(SERVICE, &self.account) {
            if entry.set_password(refresh_token).is_ok() {
                return Ok(());
            }
        }
        self.save_file(refresh_token)
    }

    pub fn load(&self) -> Result<Option<String>, AuthError> {
        if let Ok(entry) = keyring::Entry::new(SERVICE, &self.account) {
            match entry.get_password() {
                Ok(token) => return Ok(Some(token)),
                Err(keyring::Error::NoEntry) => {}
                Err(_) => {} // fall through to file
            }
        }
        match std::fs::read_to_string(&self.fallback_path) {
            Ok(token) if !token.trim().is_empty() => Ok(Some(token.trim().to_string())),
            Ok(_) => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(AuthError::Storage(e.to_string())),
        }
    }

    pub fn clear(&self) -> Result<(), AuthError> {
        if let Ok(entry) = keyring::Entry::new(SERVICE, &self.account) {
            let _ = entry.delete_credential();
        }
        match std::fs::remove_file(&self.fallback_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AuthError::Storage(e.to_string())),
        }
    }

    fn save_file(&self, refresh_token: &str) -> Result<(), AuthError> {
        if let Some(parent) = self.fallback_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AuthError::Storage(e.to_string()))?;
        }
        write_private(&self.fallback_path, refresh_token)
            .map_err(|e| AuthError::Storage(e.to_string()))
    }
}

#[cfg(unix)]
fn write_private(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    std::fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_fallback_round_trips() {
        let dir = std::env::temp_dir().join(format!("mttui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tokens.json");
        let store = TokenStore::new(path.clone());
        // exercise the file path directly (keyring may be unavailable in CI)
        store.save_file("refresh-abc").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap().trim(),
            "refresh-abc"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
