use std::path::PathBuf;

use directories::ProjectDirs;

/// Filesystem layout (see PLAN.md):
/// - config dir: config.toml, themes/*.toml, tokens.json (Phase 3 fallback)
/// - cache dir: languages/*.json (Phase 5)
/// - data dir: results.jsonl (Phase 4)
pub struct Paths {
    pub config_file: PathBuf,
    pub themes_dir: PathBuf,
    /// Fallback refresh-token file when the OS keychain is unavailable.
    pub tokens_file: PathBuf,
}

impl Paths {
    pub fn resolve() -> Option<Self> {
        let dirs = ProjectDirs::from("com", "monkeytype-tui", "monkeytype-tui")?;
        Some(Self {
            config_file: dirs.config_dir().join("config.toml"),
            themes_dir: dirs.config_dir().join("themes"),
            tokens_file: dirs.config_dir().join("tokens.json"),
        })
    }

    /// Test/fallback layout rooted at an arbitrary directory.
    pub fn rooted_at(root: &std::path::Path) -> Self {
        Self {
            config_file: root.join("config.toml"),
            themes_dir: root.join("themes"),
            tokens_file: root.join("tokens.json"),
        }
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        if let Some(parent) = self.config_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(&self.themes_dir)
    }
}
