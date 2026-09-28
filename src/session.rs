//! `$XDG_STATE_HOME/dj-tui/session.toml`: the track on each deck, so a restart puts it back.
//!
//! Small on purpose, the way `log` is. Nothing here can fail loudly: a session file that is
//! missing, unreadable or nonsense means starting with empty decks, which is where dj-tui
//! started before the file existed.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// `$XDG_STATE_HOME/dj-tui/session.toml`, falling back to `~/.local/state`.
pub fn session_path(state_home: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    let dir = match state_home.filter(|s| !s.is_empty()) {
        Some(state) => PathBuf::from(state),
        None => PathBuf::from(home?).join(".local/state"),
    };
    Some(dir.join("dj-tui").join("session.toml"))
}

/// What the last session left on the decks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct Session {
    /// Named a deck at a time rather than as an array, because TOML has no way to write the
    /// empty deck in the middle of one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deck_a: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deck_b: Option<PathBuf>,
}

impl Session {
    /// A session holding these two tracks, deck A first.
    pub fn of(decks: [Option<PathBuf>; 2]) -> Session {
        let [deck_a, deck_b] = decks;
        Session { deck_a, deck_b }
    }

    /// What was recorded, deck A first.
    pub fn decks(&self) -> [Option<PathBuf>; 2] {
        [self.deck_a.clone(), self.deck_b.clone()]
    }

    /// The recorded tracks that are still where they were. A library gets tidied between
    /// sessions, and a load of a file that has moved is an error message on startup.
    pub fn existing(&self) -> [Option<PathBuf>; 2] {
        self.decks().map(|d| d.filter(|p| p.is_file()))
    }

    /// Read `path`, or an empty session if there is nothing usable there.
    pub fn read(path: &Path) -> Session {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Write `path`, creating its directory. The caller decides whether a failure is worth
    /// saying anything about.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}
