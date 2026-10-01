//! Browser state: what was found in the music folder, what is selected, and what is typed.
//!
//! The list itself comes from `library`; this holds the part that changes as keys are pressed.

use analysis::key::Key;
use input::Dir;
use library::{compatible, scan, search, sort, Column, Entry, SearchQuery};
use std::path::{Path, PathBuf};
use tui::{BrowserRow, BrowserView};

/// How wide a tempo window the BPM filter keeps, either side of what is playing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum BpmWindow {
    #[default]
    Off,
    Near,
    Wide,
}

impl BpmWindow {
    fn next(self) -> BpmWindow {
        match self {
            BpmWindow::Off => BpmWindow::Near,
            BpmWindow::Near => BpmWindow::Wide,
            BpmWindow::Wide => BpmWindow::Off,
        }
    }

    fn tolerance(self) -> Option<f64> {
        match self {
            BpmWindow::Off => None,
            BpmWindow::Near => Some(0.03),
            BpmWindow::Wide => Some(0.06),
        }
    }

    fn label(self) -> &'static str {
        match self {
            BpmWindow::Off => "off",
            BpmWindow::Near => "\u{00b1}3%",
            BpmWindow::Wide => "\u{00b1}6%",
        }
    }
}

/// True when `bpm` sits inside `tolerance` of `against`, at the same tempo, at half time or
/// at double time. A 64 BPM track mixes against 128, and 174 mixes against 87.
fn within_window(bpm: f64, against: f64, tolerance: f64) -> bool {
    [against * 0.5, against, against * 2.0]
        .iter()
        .any(|&target| (bpm - target).abs() <= target * tolerance)
}

/// What the Alt keys narrow the list by, alongside whatever has been typed.
#[derive(Default)]
struct Filters {
    bpm: BpmWindow,
    /// The tempo the window is measured against, fixed when the filter was switched on.
    bpm_against: Option<f64>,
    key: Option<Key>,
    genre: Option<String>,
}

impl Filters {
    /// True when `entry` survives every filter that is on.
    fn keeps(&self, entry: &Entry) -> bool {
        if let (Some(tolerance), Some(against)) = (self.bpm.tolerance(), self.bpm_against) {
            match entry.bpm() {
                Some(bpm) if within_window(bpm, against, tolerance) => {}
                _ => return false,
            }
        }
        if let Some(ours) = self.key {
            match entry.key() {
                Some(theirs) if compatible(theirs, ours) => {}
                _ => return false,
            }
        }
        if let Some(want) = &self.genre {
            match entry.genre() {
                Some(genre) if genre.eq_ignore_ascii_case(want) => {}
                _ => return false,
            }
        }
        true
    }

    fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if self.bpm != BpmWindow::Off {
            chips.push(format!("\u{25b8}bpm {}", self.bpm.label()));
        }
        if self.key.is_some() {
            chips.push("\u{25b8}key".into());
        }
        if let Some(genre) = &self.genre {
            chips.push(format!("\u{25b8}genre {genre}"));
        }
        chips
    }
}

/// Split a track's display name into artist and title. A name with no artist half gives the
/// whole of it as the title, which is what a file named after the track alone looks like.
fn split_display(display: &str) -> (String, String) {
    match display.split_once(" - ") {
        Some((artist, title)) => (artist.to_string(), title.to_string()),
        None => (String::new(), display.to_string()),
    }
}

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
    filters: Filters,
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

    /// Paths whose tags have never been read.
    pub fn needs_tags(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .filter(|e| e.needs_tags())
            .map(|e| e.path().to_path_buf())
            .collect()
    }

    /// Paths with no genre that no lookup has been run against.
    pub fn needs_lookup(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .filter(|e| e.needs_lookup())
            .map(|e| e.path().to_path_buf())
            .collect()
    }

    /// The selected track's artist and title, as a lookup wants them.
    pub fn selected_artist_and_title(&self) -> Option<(String, String)> {
        let entry = self.shown.get(self.selected).map(|&i| &self.entries[i])?;
        Some(split_display(entry.display()))
    }

    /// The same for a path the backfill is working through.
    pub fn artist_and_title_of(&self, path: &Path) -> Option<(String, String)> {
        let entry = self.entries.iter().find(|e| e.path() == path)?;
        Some(split_display(entry.display()))
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
            let filters = &self.filters;
            let query = SearchQuery::new(&self.query);
            let mut scored: Vec<(i32, usize)> = self
                .shown
                .iter()
                .filter(|&&i| filters.keeps(&entries[i]))
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
        let entries = &self.entries;
        let filters = &self.filters;
        self.shown = search(entries, &self.query)
            .into_iter()
            .filter(|&i| filters.keeps(&entries[i]))
            .collect();
        self.selected = self.selected.min(self.shown.len().saturating_sub(1));
    }

    /// Cycle the BPM window. `playing` is the tempo it measures against, which is `None`
    /// with nothing playing and with a track the detector could not read.
    pub fn cycle_bpm_filter(&mut self, playing: Option<f64>) -> String {
        let Some(tempo) = playing else {
            self.filters.bpm = BpmWindow::Off;
            self.filters.bpm_against = None;
            self.refilter();
            return "BPM filter needs a deck playing with a tempo".into();
        };
        self.filters.bpm = self.filters.bpm.next();
        self.filters.bpm_against = Some(tempo);
        self.refilter();
        match self.filters.bpm {
            BpmWindow::Off => "BPM filter off".into(),
            window => format!("BPM within {} of {tempo:.1}", window.label()),
        }
    }

    pub fn toggle_key_filter(&mut self, playing: Option<Key>) -> String {
        if self.filters.key.is_some() {
            self.filters.key = None;
            self.refilter();
            return "Key filter off".into();
        }
        let Some(key) = playing else {
            return "Key filter needs a deck playing with a key".into();
        };
        self.filters.key = Some(key);
        self.refilter();
        format!("Keys that mix with {}", key.camelot())
    }

    /// Cycle the genres the library holds, alphabetically, then back to off. The list comes
    /// from every entry rather than from what is shown, so cycling does not walk a set that
    /// shrinks under it.
    pub fn cycle_genre_filter(&mut self) -> String {
        let mut genres: Vec<String> = self
            .entries
            .iter()
            .filter_map(|e| e.genre().map(str::to_string))
            .collect();
        genres.sort_by_key(|g| g.to_lowercase());
        genres.dedup_by_key(|g| g.to_lowercase());
        if genres.is_empty() {
            return "No genres yet. Alt+a reads the tags.".into();
        }
        let next = match &self.filters.genre {
            None => Some(genres[0].clone()),
            Some(current) => match genres.iter().position(|g| g == current) {
                Some(i) if i + 1 < genres.len() => Some(genres[i + 1].clone()),
                _ => None,
            },
        };
        self.filters.genre = next.clone();
        self.refilter();
        match next {
            Some(genre) => format!("Genre: {genre}"),
            None => "Genre filter off".into(),
        }
    }

    pub fn filter_chips(&self) -> Vec<String> {
        self.filters.chips()
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
                    genre: entry.genre().map(str::to_string),
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
            filters: self.filter_chips(),
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
