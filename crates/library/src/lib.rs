//! The music folder as the browser sees it: what is there, what is known about each track,
//! and which of them would mix with what is playing.
//!
//! There is no database. Everything a list shows comes from the sidecar beside each file, so a
//! library that is copied to another machine arrives complete, and playlists are plain m3u.

use analysis::key::Key;
use std::path::{Path, PathBuf};

/// Extensions the loader can decode. Opus is missing a decoder, so it is missing here too.
const AUDIO: [&str; 7] = ["flac", "wav", "mp3", "m4a", "aac", "ogg", "aiff"];

/// One track in the browser.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    path: PathBuf,
    display: String,
    bpm: Option<f64>,
    key: Option<Key>,
    duration_secs: Option<f64>,
    analysed: bool,
}

impl Entry {
    /// Read what the sidecar knows. A file with no sidecar still lists, under its own name.
    pub fn read(path: &Path) -> Entry {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let saved = loader::sidecar::Sidecar::read(path);
        let info = saved.as_ref().and_then(|s| s.track.clone());
        let analysis = saved.as_ref().and_then(|s| s.analysis.as_ref());
        let title = info
            .as_ref()
            .and_then(|i| i.title.clone())
            .unwrap_or(stem.clone());
        let display = match info.as_ref().and_then(|i| i.artist.clone()) {
            Some(artist) => format!("{artist} - {title}"),
            None => title,
        };
        let bpm = analysis.and_then(|a| a.grid.map(|g| g.bpm));
        Entry {
            path: path.to_path_buf(),
            display,
            bpm,
            key: analysis.and_then(|a| a.key),
            duration_secs: info.and_then(|i| i.duration_secs),
            // A sidecar with no tempo in it is worth another run: the file may be one the
            // detector could not read the first time round.
            analysed: bpm.is_some(),
        }
    }

    /// Build an entry directly, for tests and for a list that came from somewhere else.
    pub fn for_test(
        path: &Path,
        display: String,
        bpm: Option<f64>,
        key: Option<Key>,
        duration_secs: Option<f64>,
    ) -> Entry {
        Entry {
            path: path.to_path_buf(),
            display,
            bpm,
            key,
            duration_secs,
            analysed: bpm.is_some(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn display(&self) -> &str {
        &self.display
    }

    pub fn bpm(&self) -> Option<f64> {
        self.bpm
    }

    pub fn key(&self) -> Option<Key> {
        self.key
    }

    pub fn duration_secs(&self) -> Option<f64> {
        self.duration_secs
    }

    /// True once the track has been through analysis and has a sidecar to show for it.
    pub fn analysed(&self) -> bool {
        self.analysed
    }
}

/// Walk `roots` for audio files, newest folders and all, and read what is known about each.
pub fn scan(roots: &[PathBuf]) -> Vec<Entry> {
    let mut found = Vec::new();
    let mut stack: Vec<PathBuf> = roots.to_vec();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if is_audio(&path) {
                found.push(Entry::read(&path));
            }
        }
    }
    sort(&mut found, Column::Name, true);
    found
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO.contains(&e.to_ascii_lowercase().as_str()))
}

/// The columns a track list can be ordered by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Column {
    #[default]
    Name,
    Bpm,
    Key,
    Length,
}

impl Column {
    pub fn next(self) -> Column {
        match self {
            Column::Name => Column::Bpm,
            Column::Bpm => Column::Key,
            Column::Key => Column::Length,
            Column::Length => Column::Name,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Column::Name => "name",
            Column::Bpm => "BPM",
            Column::Key => "key",
            Column::Length => "length",
        }
    }
}

/// Order `list` by `column`. Tracks with nothing in that column sink to the bottom either
/// way round, since a list of unknowns at the top is no use to anybody.
pub fn sort(list: &mut [Entry], column: Column, ascending: bool) {
    list.sort_by(|a, b| {
        let known = |e: &Entry| match column {
            Column::Name => true,
            Column::Bpm => e.bpm.is_some(),
            Column::Key => e.key.is_some(),
            Column::Length => e.duration_secs.is_some(),
        };
        match (known(a), known(b)) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            (false, false) => return a.display.cmp(&b.display),
            (true, true) => {}
        }
        let ordering = match column {
            Column::Name => a.display.to_lowercase().cmp(&b.display.to_lowercase()),
            Column::Bpm => a
                .bpm
                .unwrap_or_default()
                .total_cmp(&b.bpm.unwrap_or_default()),
            Column::Length => a
                .duration_secs
                .unwrap_or_default()
                .total_cmp(&b.duration_secs.unwrap_or_default()),
            // Camelot order, so neighbours on the wheel sit together.
            Column::Key => camelot_order(a.key).cmp(&camelot_order(b.key)),
        };
        if ascending {
            ordering
        } else {
            ordering.reverse()
        }
    });
}

fn camelot_order(key: Option<Key>) -> (u32, char) {
    match key {
        Some(k) => {
            let code = k.camelot();
            let letter = code.chars().last().unwrap_or('A');
            let number = code.trim_end_matches(letter).parse().unwrap_or(0);
            (number, letter)
        }
        None => (u32::MAX, 'Z'),
    }
}

/// Indices of the entries matching `query`, best first. Letters have to appear in order but
/// need not be next to each other, which is what makes it worth typing three of them.
pub fn search(list: &[Entry], query: &str) -> Vec<usize> {
    if query.trim().is_empty() {
        return (0..list.len()).collect();
    }
    let mut scored: Vec<(i32, usize)> = list
        .iter()
        .enumerate()
        .filter_map(|(i, entry)| score(query, entry.display()).map(|s| (s, i)))
        .collect();
    // Best score first, and the earlier entry wins a tie so the order stays predictable.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

/// How well `text` matches `query`, or `None` if it does not. Letters in a row and letters
/// starting a word both count for more, which puts the obvious match at the top.
pub fn score(query: &str, text: &str) -> Option<i32> {
    let needle: Vec<char> = query
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if needle.is_empty() {
        return Some(0);
    }
    let haystack: Vec<char> = text.to_lowercase().chars().collect();
    let mut total = 0;
    let mut run = 0;
    let mut at = 0;
    for want in needle {
        let found = haystack[at..].iter().position(|c| *c == want)? + at;
        let starts_word = found == 0 || !haystack[found - 1].is_alphanumeric();
        run = if found == at && at > 0 { run + 1 } else { 0 };
        total += 10 + run * 5 + if starts_word { 8 } else { 0 };
        // A match near the front of the name is usually the one meant.
        total -= (found as i32).min(20) / 4;
        at = found + 1;
    }
    Some(total)
}

/// True when two keys sit next to each other on the Camelot wheel, or are the relative
/// major and minor of one another, which is what DJs mix between.
pub fn compatible(a: Key, b: Key) -> bool {
    let (an, al) = camelot_parts(a);
    let (bn, bl) = camelot_parts(b);
    if al == bl {
        let step = (an as i32 - bn as i32).rem_euclid(12);
        step == 0 || step == 1 || step == 11
    } else {
        an == bn
    }
}

fn camelot_parts(key: Key) -> (u32, char) {
    let (number, letter) = camelot_order(Some(key));
    (number % 12, letter)
}

/// An m3u playlist: the paths in it that still exist, in the order they were written.
pub struct Playlist {
    name: String,
    entries: Vec<Entry>,
}

impl Playlist {
    /// Read `path`, resolving relative lines against the file's own directory.
    pub fn read(path: &Path) -> std::io::Result<Playlist> {
        let text = std::fs::read_to_string(path)?;
        let base = path.parent().unwrap_or(Path::new("."));
        let entries = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                let p = PathBuf::from(line);
                if p.is_absolute() {
                    p
                } else {
                    base.join(p)
                }
            })
            .filter(|p| p.is_file())
            .map(|p| Entry::read(&p))
            .collect();
        Ok(Playlist {
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            entries,
        })
    }

    pub fn from_entries(name: String, entries: Vec<Entry>) -> Playlist {
        Playlist { name, entries }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Write the playlist out, with the title line other players expect.
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        let mut text = String::from("#EXTM3U\n");
        for entry in &self.entries {
            let secs = entry.duration_secs().unwrap_or(0.0).round() as i64;
            text.push_str(&format!("#EXTINF:{secs},{}\n", entry.display()));
            text.push_str(&entry.path().display().to_string());
            text.push('\n');
        }
        std::fs::write(path, text)
    }
}
