//! Browser state: what was found in the music folder, what is selected, and what is typed.
//!
//! The list itself comes from `library`; this holds the part that changes as keys are pressed.

use analysis::key::Key;
use input::Dir;
use library::{compatible, scan, search, sort, Column, Entry, SearchQuery};
use std::path::{Path, PathBuf};
use tui::{BrowserRow, BrowserView};

/// A terminal can show only a small part of the library at once. Keep the per-frame view
/// bounded, centred on the selection, instead of formatting every track on every frame.
const BROWSER_VIEW_ROWS: usize = 256;

#[derive(Default)]
pub struct Browser {
    entries: Vec<Entry>,
    /// Indices into `entries` that the query left, in the order to show them.
    shown: Vec<usize>,
    selected: usize,
    column: Column,
    ascending: bool,
    /// The query that narrows the list. The keymap's mode says who owns the keyboard.
    query: String,
    pub fullscreen: bool,
    /// What the panel says about itself: how many tracks, or what it is busy with.
    note: Option<String>,
}

impl Browser {
    pub fn new() -> Browser {
        Browser {
            ascending: true,
            ..Default::default()
        }
    }

    /// Read `roots` and show what is there.
    pub fn scan(&mut self, roots: &[PathBuf]) {
        self.entries = scan(roots);
        sort(&mut self.entries, self.column, self.ascending);
        self.note = None;
        self.refilter();
    }

    /// Replace the listing, for a playlist or a rescan that happened elsewhere.
    pub fn set_entries(&mut self, entries: Vec<Entry>) {
        self.entries = entries;
        sort(&mut self.entries, self.column, self.ascending);
        self.refilter();
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Paths in the list that have never been analysed.
    pub fn unanalysed(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .filter(|e| !e.analysed())
            .map(|e| e.path().to_path_buf())
            .collect()
    }

    /// Re-read one file's sidecar, for a track that has just been analysed.
    pub fn refresh(&mut self, path: &Path) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.path() == path) {
            *entry = Entry::read(path);
        }
    }

    /// Say what the browser is busy with, or `None` to go back to counting tracks.
    pub fn set_note(&mut self, note: Option<String>) {
        self.note = note;
    }

    pub fn selected_path(&self) -> Option<&Path> {
        self.shown
            .get(self.selected)
            .map(|&i| self.entries[i].path())
    }

    pub fn move_selection(&mut self, dir: Dir) {
        if self.shown.is_empty() {
            return;
        }
        self.selected = match dir {
            Dir::Down => (self.selected + 1).min(self.shown.len() - 1),
            Dir::Up => self.selected.saturating_sub(1),
        };
    }

    /// Cycle the sort column, or with `reverse` turn the order around where it is.
    pub fn sort_by(&mut self, reverse: bool) {
        if reverse {
            self.ascending = !self.ascending;
        } else {
            self.column = self.column.next();
        }
        sort(&mut self.entries, self.column, self.ascending);
        self.refilter();
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// Empty the query and put the whole list back.
    pub fn clear_query(&mut self) {
        self.query.clear();
        self.refilter();
    }

    pub fn type_char(&mut self, c: char) {
        self.query.push(c);
        // The shortcut below holds only while a longer query matches a subset of what the
        // shorter one did. A field token breaks that: `bp` is a bare word matching almost
        // nothing, and `bpm:124` has to find the tracks `bp` threw away.
        if self.query.contains(':') {
            self.refilter();
            return;
        }
        // Every match for the longer query must also match the query before this character.
        // Re-score those candidates instead of scanning the whole library for every key.
        let matches = {
            let entries = &self.entries;
            let query = SearchQuery::new(&self.query);
            let mut scored: Vec<(i32, usize)> = self
                .shown
                .iter()
                .filter_map(|&i| query.score_entry(&entries[i]).map(|s| (s, i)))
                .collect();
            scored.sort_by(|(a_score, a_index), (b_score, b_index)| {
                b_score.cmp(a_score).then(a_index.cmp(b_index))
            });
            scored.into_iter().map(|(_, i)| i).collect()
        };
        self.shown = matches;
        self.selected = self.selected.min(self.shown.len().saturating_sub(1));
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.refilter();
    }

    fn refilter(&mut self) {
        self.shown = search(&self.entries, &self.query);
        self.selected = self.selected.min(self.shown.len().saturating_sub(1));
    }

    /// The panel. `active` is true while browser mode holds the keyboard, and `playing` is
    /// the key of whatever is playing, for the highlighting.
    pub fn view(&self, active: bool, playing: Option<Key>) -> BrowserView {
        let first = self
            .selected
            .saturating_sub(BROWSER_VIEW_ROWS / 2)
            .min(self.shown.len().saturating_sub(BROWSER_VIEW_ROWS));
        let rows = self.shown[first..]
            .iter()
            .take(BROWSER_VIEW_ROWS)
            .map(|&i| {
                let entry = &self.entries[i];
                BrowserRow {
                    name: entry.display().to_string(),
                    bpm: entry.bpm(),
                    key: entry.key().map(|k| k.camelot()),
                    duration_secs: entry.duration_secs(),
                    compatible: match (entry.key(), playing) {
                        (Some(theirs), Some(ours)) => compatible(theirs, ours),
                        _ => false,
                    },
                    analysed: entry.analysed(),
                }
            })
            .collect();
        BrowserView {
            rows,
            selected: self.selected - first,
            sort: self.column.label(),
            ascending: self.ascending,
            // Drawn whenever it is filtering, so leaving the mode cannot hide a filter.
            search: (active || !self.query.is_empty()).then(|| self.query.clone()),
            active,
            fullscreen: self.fullscreen,
            status: self.status(),
        }
    }

    fn status(&self) -> String {
        if let Some(note) = &self.note {
            return note.clone();
        }
        match (self.entries.len(), self.shown.len()) {
            (0, _) => "no tracks".into(),
            (total, shown) if shown == total => format!("{total} tracks"),
            (total, shown) => format!("{shown} of {total} tracks"),
        }
    }
}
