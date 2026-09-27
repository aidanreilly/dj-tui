//! Browser state: what was found in the music folder, what is selected, and what is typed.
//!
//! The list itself comes from `library`; this holds the part that changes as keys are pressed.

use analysis::key::Key;
use input::Dir;
use library::{compatible, scan, search, sort, Column, Entry};
use std::path::{Path, PathBuf};
use tui::{BrowserRow, BrowserView};

#[derive(Default)]
pub struct Browser {
    entries: Vec<Entry>,
    /// Indices into `entries` that the query left, in the order to show them.
    shown: Vec<usize>,
    selected: usize,
    column: Column,
    ascending: bool,
    /// The query. `typing` says whether the prompt is open.
    query: String,
    typing: bool,
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

    pub fn typing(&self) -> bool {
        self.typing
    }

    /// Open the search prompt, starting from nothing.
    pub fn start_search(&mut self) {
        self.typing = true;
        self.query.clear();
        self.refilter();
    }

    pub fn type_char(&mut self, c: char) {
        self.query.push(c);
        self.refilter();
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.refilter();
    }

    /// Close the prompt and keep what it found.
    pub fn accept_search(&mut self) {
        self.typing = false;
    }

    /// Close the prompt and forget the query.
    pub fn cancel_search(&mut self) {
        self.typing = false;
        self.query.clear();
        self.refilter();
    }

    fn refilter(&mut self) {
        self.shown = search(&self.entries, &self.query);
        self.selected = self.selected.min(self.shown.len().saturating_sub(1));
    }

    /// The panel, with `playing` the key of whatever is playing for the highlighting.
    pub fn view(&self, playing: Option<Key>) -> BrowserView {
        let rows = self
            .shown
            .iter()
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
            selected: self.selected,
            sort: self.column.label(),
            ascending: self.ascending,
            search: self.typing.then(|| self.query.clone()),
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
