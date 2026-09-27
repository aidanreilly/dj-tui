//! The log file: where it goes, what a line looks like, and how it is kept from growing.

use dj_tui::log::{log_path, stamp, Log};
use std::path::PathBuf;

#[test]
fn the_log_follows_the_state_directory() {
    assert_eq!(
        log_path(Some("/state"), Some("/home/dj")),
        Some(PathBuf::from("/state/dj-tui/dj-tui.log"))
    );
    assert_eq!(
        log_path(None, Some("/home/dj")),
        Some(PathBuf::from("/home/dj/.local/state/dj-tui/dj-tui.log")),
        "the default when XDG_STATE_HOME is unset"
    );
    assert_eq!(log_path(None, None), None, "nowhere to put it");
    assert!(
        log_path(Some(""), Some("/home/dj")).unwrap().is_absolute(),
        "an empty XDG_STATE_HOME falls back rather than making a relative path"
    );
}

#[test]
fn a_line_carries_the_time_and_the_text() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dj-tui.log");
    let mut log = Log::open(&path);
    log.line("started");
    log.line("loaded Warehouse Tool on deck A");
    drop(log);

    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("started"), "{}", lines[0]);
    assert!(lines[1].contains("deck A"), "{}", lines[1]);
    // 2026-09-27 18:42:05 started
    let time = &lines[0][..19];
    assert_eq!(time.len(), 19);
    assert!(
        time.starts_with("20") && time.chars().filter(|c| *c == ':').count() == 2,
        "{time}"
    );
}

#[test]
fn a_second_run_adds_to_the_file_rather_than_replacing_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dj-tui.log");
    Log::open(&path).line("first run");
    Log::open(&path).line("second run");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("first run") && text.contains("second run"),
        "{text}"
    );
}

#[test]
fn a_log_that_has_grown_too_big_starts_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dj-tui.log");
    std::fs::write(&path, "x".repeat(2_000_000)).unwrap();
    let mut log = Log::open(&path);
    log.line("after the rotation");
    drop(log);

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.len() < 1_000,
        "the new log is fresh: {} bytes",
        text.len()
    );
    assert!(text.contains("after the rotation"));
    let old = std::fs::read_to_string(path.with_extension("log.old")).unwrap();
    assert_eq!(old.len(), 2_000_000, "and the last one is kept beside it");
}

#[test]
fn a_log_that_cannot_be_written_is_not_a_reason_to_stop() {
    // A directory where the file should go: opening it can only fail.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dj-tui.log");
    std::fs::create_dir(&path).unwrap();
    let mut log = Log::open(&path);
    log.line("goes nowhere, quietly");
}

#[test]
fn timestamps_read_as_dates() {
    assert_eq!(stamp(0), "1970-01-01 00:00:00");
    assert_eq!(stamp(1_000_000_000), "2001-09-09 01:46:40");
    assert_eq!(stamp(1_788_000_000), "2026-08-29 10:40:00");
    assert_eq!(stamp(951_782_400), "2000-02-29 00:00:00", "a leap day");
}
