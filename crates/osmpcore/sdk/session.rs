use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Session {
    /// Last .osmp file opened with `osmp bench`
    pub last_file: Option<String>,
}

impl Session {
    fn path() -> Option<PathBuf> {
        // Windows: %APPDATA%\osmp\session.json
        // Linux / macOS: ~/.config/osmp/session.json
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|_| PathBuf::from("."));
        Some(base.join("osmp").join("session.json"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| fs::read_to_string(&p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = Self::path() {
            if let Some(dir) = p.parent() {
                let _ = fs::create_dir_all(dir);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(&p, json);
            }
        }
    }
}
