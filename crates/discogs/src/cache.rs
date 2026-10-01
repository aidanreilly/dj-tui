use crate::release::normalise;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// What has already been asked of Discogs, so nothing is asked twice.
///
/// The sidecar beside each track records that a file was looked up, which covers that file.
/// This covers the question instead: the same record ripped twice, or moved, or re-tagged,
/// is one lookup rather than one per copy. A `None` against a key means it was asked and
/// nothing matched, which is an answer worth keeping: without it every run would send the
/// same fruitless request again.
#[derive(Debug)]
pub struct Cache {
    path: PathBuf,
    entries: HashMap<String, Option<String>>,
}

fn key(artist: &str, title: &str) -> String {
    format!("{}|{}", normalise(artist), normalise(title))
}

impl Cache {
    /// Read `path`. A missing or unreadable file is an empty cache: this is a saving, not a
    /// source of truth, and failing to start over it would be absurd.
    pub fn load(path: &Path) -> Cache {
        let entries = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Cache {
            path: path.to_path_buf(),
            entries,
        }
    }

    /// `None` when the question has never been asked. `Some(None)` when it was asked and
    /// nothing matched.
    pub fn get(&self, artist: &str, title: &str) -> Option<Option<String>> {
        self.entries.get(&key(artist, title)).cloned()
    }

    pub fn insert(&mut self, artist: &str, title: &str, genre: Option<String>) {
        self.entries.insert(key(artist, title), genre);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn save(&self) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(&self.path, text)
    }
}
