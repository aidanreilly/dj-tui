//! The browser as the app drives it: scanning, moving, searching, sorting and loading.

mod common;
use common::{stereo, write_wav, Fmt};
use dj_tui::app::App;
use dj_tui::config::Config;
use engine::{channel, Engine, EngineProcessor};
use input::{Key, KeyEvent};
use std::path::Path;
use std::time::{Duration, Instant};

const RATE: u32 = 48_000;

fn setup() -> (App, EngineProcessor) {
    let (handle, processor) = channel(Engine::new(), 64);
    (App::new(handle, &Config::default(), RATE), processor)
}

fn process(p: &mut EngineProcessor, frames: usize) {
    let (mut m, mut c) = (vec![0.0; frames * 2], vec![0.0; frames * 2]);
    p.process(&mut m, &mut c);
}

fn music(dir: &Path, names: &[&str]) {
    for name in names {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_wav(&path, &stereo(&[0.2; 4_800]), 2, RATE, Fmt::Pcm16);
    }
}

fn press(app: &mut App, key: Key) {
    app.on_key(KeyEvent::press(key));
}

#[test]
fn the_browser_lists_the_music_folder() {
    let dir = tempfile::tempdir().unwrap();
    music(dir.path(), &["Alpha.wav", "deep/Beta.wav"]);
    let (mut app, _p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    let view = app.view(String::new());
    let names: Vec<&str> = view.browser.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["Alpha", "Beta"]);
    assert!(view.browser.status.contains('2'), "{}", view.browser.status);
}

#[test]
fn the_arrows_move_the_selection_and_stop_at_the_ends() {
    let dir = tempfile::tempdir().unwrap();
    music(dir.path(), &["Alpha.wav", "Beta.wav", "Gamma.wav"]);
    let (mut app, _p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    assert_eq!(app.view(String::new()).browser.selected, 0);
    press(&mut app, Key::Down);
    press(&mut app, Key::Down);
    assert_eq!(app.view(String::new()).browser.selected, 2);
    press(&mut app, Key::Down);
    assert_eq!(
        app.view(String::new()).browser.selected,
        2,
        "the last is the last"
    );
    for _ in 0..5 {
        press(&mut app, Key::Up);
    }
    assert_eq!(app.view(String::new()).browser.selected, 0);
}

#[test]
fn enter_loads_the_selected_track_onto_the_focused_deck() {
    let dir = tempfile::tempdir().unwrap();
    music(dir.path(), &["Alpha.wav", "Beta.wav"]);
    let (mut app, mut p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    press(&mut app, Key::Down);
    press(&mut app, Key::Tab); // focus deck B
    press(&mut app, Key::Enter);

    let start = Instant::now();
    while app.snapshot().decks[1].track_frames == 0 {
        app.tick();
        process(&mut p, 16);
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{}",
            app.message()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        app.view(String::new()).decks[1].title.as_deref(),
        Some("Beta")
    );
}

#[test]
fn typing_after_a_slash_filters_the_list_as_it_goes() {
    let dir = tempfile::tempdir().unwrap();
    music(dir.path(), &["Warehouse Tool.wav", "Breakdown Edit.wav"]);
    let (mut app, _p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);

    press(&mut app, Key::Char('/'));
    assert_eq!(
        app.view(String::new()).browser.search.as_deref(),
        Some(""),
        "the prompt is open"
    );
    for c in "ware".chars() {
        press(&mut app, Key::Char(c));
    }
    let view = app.view(String::new());
    assert_eq!(view.browser.search.as_deref(), Some("ware"));
    assert_eq!(view.browser.rows.len(), 1, "only the matching track");
    assert_eq!(view.browser.rows[0].name, "Warehouse Tool");

    press(&mut app, Key::Backspace);
    assert_eq!(
        app.view(String::new()).browser.search.as_deref(),
        Some("war")
    );

    press(&mut app, Key::Esc);
    let view = app.view(String::new());
    assert!(view.browser.search.is_none(), "the prompt is gone");
    assert_eq!(view.browser.rows.len(), 2, "and the whole list is back");
}

#[test]
fn a_search_that_is_accepted_keeps_its_results() {
    let dir = tempfile::tempdir().unwrap();
    music(dir.path(), &["Warehouse Tool.wav", "Breakdown Edit.wav"]);
    let (mut app, _p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    press(&mut app, Key::Char('/'));
    for c in "ware".chars() {
        press(&mut app, Key::Char(c));
    }
    press(&mut app, Key::Enter);
    let view = app.view(String::new());
    assert!(view.browser.search.is_none(), "typing is over");
    assert_eq!(view.browser.rows.len(), 1, "the filter stays");
}

#[test]
fn keys_do_their_usual_work_again_once_the_search_is_over() {
    let dir = tempfile::tempdir().unwrap();
    music(dir.path(), &["Alpha.wav"]);
    let (mut app, _p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    press(&mut app, Key::Char('/'));
    press(&mut app, Key::Char('w'));
    assert_eq!(
        app.view(String::new()).decks[0].waveform_mode,
        Default::default(),
        "w typed into the search box is a letter, not a command"
    );
    press(&mut app, Key::Esc);
    press(&mut app, Key::Char('w'));
    assert_ne!(
        app.view(String::new()).decks[0].waveform_mode,
        Default::default(),
        "and a command again afterwards"
    );
}

#[test]
fn the_sort_key_cycles_the_columns_and_reverses() {
    let dir = tempfile::tempdir().unwrap();
    music(dir.path(), &["Alpha.wav"]);
    let (mut app, _p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    assert_eq!(app.view(String::new()).browser.sort, "name");
    press(&mut app, Key::Char('S'));
    assert_eq!(app.view(String::new()).browser.sort, "BPM");
    assert!(app.view(String::new()).browser.ascending);
    app.on_key(KeyEvent::press(Key::Char('S')).alt());
    assert!(
        !app.view(String::new()).browser.ascending,
        "alt turns the order around without moving column"
    );
    assert_eq!(app.view(String::new()).browser.sort, "BPM");
}

#[test]
fn b_gives_the_browser_the_whole_screen_and_gives_it_back() {
    let (mut app, _p) = setup();
    assert!(!app.view(String::new()).browser.fullscreen);
    press(&mut app, Key::Char('b'));
    assert!(app.view(String::new()).browser.fullscreen);
    press(&mut app, Key::Char('b'));
    assert!(!app.view(String::new()).browser.fullscreen);
}

#[test]
fn a_track_that_would_mix_with_the_playing_deck_is_marked() {
    let dir = tempfile::tempdir().unwrap();
    music(dir.path(), &["Alpha.wav"]);
    let (mut app, _p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    // Nothing is playing and nothing is analysed, so nothing is marked yet.
    assert!(app
        .view(String::new())
        .browser
        .rows
        .iter()
        .all(|r| !r.compatible));
}
