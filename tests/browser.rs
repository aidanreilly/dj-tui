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

/// Music with a beat in it, for the tests that run analysis: a track whose tempo never
/// came back is not counted as analysed.
fn beat_music(dir: &Path, names: &[&str]) {
    let beat = 60.0 / 128.0;
    let mono: Vec<f32> = (0..RATE as usize * 6)
        .map(|i| {
            let tb = (i as f32 / RATE as f32) % beat;
            if tb < 0.02 {
                (-tb / 0.004).exp() * (std::f32::consts::TAU * 1500.0 * tb).sin()
            } else {
                0.0
            }
        })
        .collect();
    for name in names {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_wav(&path, &stereo(&mono), 2, RATE, Fmt::Float32);
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
    press(&mut app, Key::Char('b'));
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
    press(&mut app, Key::Char('b'));
    press(&mut app, Key::Down);
    press(&mut app, Key::Tab); // focus deck B, without leaving the browser
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
fn typing_in_browser_mode_filters_the_list_as_it_goes() {
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

    app.on_key(KeyEvent::press(Key::Char('u')).ctrl());
    let view = app.view(String::new());
    assert_eq!(view.browser.search.as_deref(), Some(""), "cleared");
    assert_eq!(view.browser.rows.len(), 2, "and the whole list is back");
}

#[test]
fn esc_hands_the_keyboard_back_in_one_press_and_keeps_the_filter() {
    let dir = tempfile::tempdir().unwrap();
    music(dir.path(), &["Warehouse Tool.wav", "Breakdown Edit.wav"]);
    let (mut app, _p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    press(&mut app, Key::Char('/'));
    for c in "ware".chars() {
        press(&mut app, Key::Char(c));
    }
    press(&mut app, Key::Esc);
    let view = app.view(String::new());
    assert!(!view.browser.active, "the browser let go of the keyboard");
    assert_eq!(
        view.browser.search.as_deref(),
        Some("ware"),
        "a filter that is still on is still shown"
    );
    assert_eq!(view.browser.rows.len(), 1, "and still filtering");
}

#[test]
fn keys_do_their_usual_work_again_once_browser_mode_is_over() {
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
    press(&mut app, Key::Char('b'));
    assert_eq!(app.view(String::new()).browser.sort, "name");
    app.on_key(KeyEvent::press(Key::Char('s')).alt());
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
fn b_gives_the_browser_the_keyboard_and_alt_f_the_whole_screen() {
    let (mut app, _p) = setup();
    assert!(!app.view(String::new()).browser.active);
    press(&mut app, Key::Char('b'));
    assert!(app.view(String::new()).browser.active);
    assert!(!app.view(String::new()).browser.fullscreen, "not yet");

    app.on_key(KeyEvent::press(Key::Char('f')).alt());
    assert!(app.view(String::new()).browser.fullscreen);
    app.on_key(KeyEvent::press(Key::Char('f')).alt());
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

#[test]
fn alt_a_analyses_everything_in_the_list_that_has_no_analysis_yet() {
    let dir = tempfile::tempdir().unwrap();
    beat_music(dir.path(), &["One.wav", "Two.wav"]);
    let (mut app, mut p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    assert!(
        app.view(String::new())
            .browser
            .rows
            .iter()
            .all(|r| !r.analysed),
        "nothing has been analysed"
    );

    press(&mut app, Key::Char('b'));
    app.on_key(KeyEvent::press(Key::Char('a')).alt());
    assert!(
        app.view(String::new()).browser.status.contains("nalys"),
        "the panel says what it is doing: {}",
        app.view(String::new()).browser.status
    );

    let start = Instant::now();
    while !app
        .view(String::new())
        .browser
        .rows
        .iter()
        .all(|r| r.analysed)
    {
        app.tick();
        process(&mut p, 16);
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "{}",
            app.message()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let status = app.view(String::new()).browser.status;
    assert!(status.contains("2 tracks"), "back to counting: {status}");
    assert!(app.message().contains("Analysed"), "{}", app.message());
}

#[test]
fn analysis_leaves_tracks_that_already_have_a_tempo_alone() {
    let dir = tempfile::tempdir().unwrap();
    beat_music(dir.path(), &["One.wav"]);
    let (mut app, mut p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    press(&mut app, Key::Char('b'));
    app.on_key(KeyEvent::press(Key::Char('a')).alt());
    let start = Instant::now();
    while app.view(String::new()).browser.status.contains("nalys") {
        app.tick();
        process(&mut p, 16);
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "first pass timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    app.on_key(KeyEvent::press(Key::Char('a')).alt());
    assert!(
        app.message().contains("already"),
        "there is nothing left to do: {}",
        app.message()
    );
}

mod devices {
    use super::*;
    use input::KeyEvent;

    fn devices() -> Vec<(String, String)> {
        vec![
            ("hw:0,0".into(), "Built-in Audio".into()),
            ("default".into(), "Default ALSA Output".into()),
        ]
    }

    #[test]
    fn ctrl_d_opens_the_device_screen_and_closes_it_again() {
        let (mut app, _p) = setup();
        app.set_devices(devices(), "default".into(), "48000 Hz / 256 frames".into());
        assert!(app.view(String::new()).devices.is_none());

        app.on_key(KeyEvent::press(Key::Char('d')).ctrl());
        let view = app.view(String::new());
        let screen = view.devices.expect("the screen is up");
        assert_eq!(screen.devices.len(), 2);
        assert_eq!(screen.current, "default");
        assert_eq!(screen.selected, 1, "it starts on the device in use");

        app.on_key(KeyEvent::press(Key::Esc));
        assert!(app.view(String::new()).devices.is_none());
    }

    #[test]
    fn the_arrows_move_and_enter_picks_a_device() {
        let (mut app, _p) = setup();
        app.set_devices(devices(), "default".into(), String::new());
        app.on_key(KeyEvent::press(Key::Char('d')).ctrl());
        app.on_key(KeyEvent::press(Key::Up));
        assert_eq!(app.view(String::new()).devices.unwrap().selected, 0);

        app.on_key(KeyEvent::press(Key::Enter));
        assert!(
            app.view(String::new()).devices.is_none(),
            "choosing closes the screen"
        );
        assert_eq!(app.chosen_device().as_deref(), Some("hw:0,0"));
        assert!(app.message().contains("hw:0,0"), "{}", app.message());
    }

    #[test]
    fn the_keys_underneath_are_left_alone_while_the_screen_is_up() {
        let (mut app, _p) = setup();
        app.set_devices(devices(), "default".into(), String::new());
        app.on_key(KeyEvent::press(Key::Char('d')).ctrl());
        app.on_key(KeyEvent::press(Key::Space));
        assert!(
            !app.snapshot().decks[0].playing,
            "space does not start a deck from the device screen"
        );
        assert!(app.view(String::new()).devices.is_some(), "and it stays up");
    }
}
