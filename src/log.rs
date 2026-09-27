//! A log file, so a session that went wrong leaves something behind.
//!
//! Deliberately small: append a timestamped line, start again when the file gets large, and
//! never let a logging problem reach the person mixing. Nothing here is on the audio thread.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Past this the log is moved aside and a new one started.
const MAX_BYTES: u64 = 1_000_000;

/// `$XDG_STATE_HOME/dj-tui/dj-tui.log`, falling back to `~/.local/state`.
pub fn log_path(state_home: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    let dir = match state_home.filter(|s| !s.is_empty()) {
        Some(state) => PathBuf::from(state),
        None => PathBuf::from(home?).join(".local/state"),
    };
    Some(dir.join("dj-tui").join("dj-tui.log"))
}

/// An open log, or nothing at all if it could not be opened.
pub struct Log {
    file: Option<File>,
}

impl Log {
    /// Open `path` for appending, rotating it first if it has grown past `MAX_BYTES`.
    /// Any failure leaves a log that quietly swallows what it is given.
    pub fn open(path: &Path) -> Log {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() > MAX_BYTES) {
            let _ = std::fs::rename(path, path.with_extension("log.old"));
        }
        Log {
            file: OpenOptions::new().create(true).append(true).open(path).ok(),
        }
    }

    /// A log that writes nowhere, for `--no-audio` style runs and for tests.
    pub fn none() -> Log {
        Log { file: None }
    }

    pub fn line(&mut self, text: &str) {
        let Some(file) = &mut self.file else { return };
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let _ = writeln!(file, "{} {text}", stamp(secs));
        let _ = file.flush();
    }
}

/// Unix seconds as `YYYY-MM-DD HH:MM:SS` in UTC. Days to a civil date by Howard Hinnant's
/// algorithm, which is shorter than pulling in a date library for one line of output.
pub fn stamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let time = secs.rem_euclid(86_400);
    let (hour, minute, second) = (time / 3_600, (time % 3_600) / 60, time % 60);

    // Shift the era so the leap day lands at the end of the year.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}
