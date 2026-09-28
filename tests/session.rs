//! The session file: what was on each deck, so a restart puts it back.

use dj_tui::session::{session_path, Session};
use std::path::{Path, PathBuf};

fn touch(path: &Path) -> PathBuf {
    std::fs::write(path, b"").unwrap();
    path.to_path_buf()
}

#[test]
fn the_session_file_sits_beside_the_log() {
    assert_eq!(
        session_path(Some("/state"), Some("/home/dj")).unwrap(),
        Path::new("/state/dj-tui/session.toml")
    );
    assert_eq!(
        session_path(None, Some("/home/dj")).unwrap(),
        Path::new("/home/dj/.local/state/dj-tui/session.toml")
    );
    assert_eq!(session_path(None, None), None);
}

#[test]
fn what_was_on_the_decks_is_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let a = touch(&dir.path().join("one.wav"));
    let b = touch(&dir.path().join("two.wav"));
    // A state directory that is not there yet is the ordinary case on a first run.
    let path = dir.path().join("state").join("session.toml");

    Session::of([Some(a.clone()), Some(b.clone())])
        .write(&path)
        .unwrap();

    assert_eq!(Session::read(&path).decks(), [Some(a), Some(b)]);
}

#[test]
fn a_deck_that_was_empty_stays_empty() {
    let dir = tempfile::tempdir().unwrap();
    let b = touch(&dir.path().join("two.wav"));
    let path = dir.path().join("session.toml");

    Session::of([None, Some(b.clone())]).write(&path).unwrap();

    assert_eq!(Session::read(&path).decks(), [None, Some(b)]);
}

#[test]
fn no_session_file_means_no_tracks() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        Session::read(&dir.path().join("never-written.toml")).decks(),
        [None, None]
    );
}

#[test]
fn a_file_that_will_not_parse_is_no_tracks_rather_than_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.toml");
    std::fs::write(&path, "deck_a = [not toml").unwrap();
    assert_eq!(Session::read(&path).decks(), [None, None]);
}

#[test]
fn a_track_that_has_moved_since_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let a = touch(&dir.path().join("one.wav"));
    let gone = dir.path().join("gone.wav");
    let session = Session::of([Some(a.clone()), Some(gone.clone())]);

    assert_eq!(
        session.decks(),
        [Some(a.clone()), Some(gone)],
        "what was recorded is still what was recorded"
    );
    assert_eq!(
        session.existing(),
        [Some(a), None],
        "only the ones still there can be loaded"
    );
}
