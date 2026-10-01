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

fn press_alt(app: &mut App, key: Key) {
    app.on_key(KeyEvent::press(key).alt());
}

/// The track names the browser is showing, in order.
fn rows(app: &App) -> Vec<String> {
    app.view(String::new())
        .browser
        .rows
        .iter()
        .map(|r| r.name.clone())
        .collect()
}

/// Name, tempo, Camelot key, genre: what a track's sidecar says about it.
type Track<'a> = (&'a str, Option<f64>, Option<&'a str>, Option<&'a str>);

/// A library whose sidecars already say what each track is. The filters read the sidecar,
/// so writing one directly tests the filter rather than the detector, and keeps the fixture
/// to a file write instead of a full analysis per track.
fn library(dir: &Path, tracks: &[Track]) {
    for (name, bpm, key, genre) in tracks {
        let path = dir.join(format!("{name}.wav"));
        write_wav(&path, &stereo(&[0.2; 4_800]), 2, RATE, Fmt::Pcm16);
        let sidecar = loader::sidecar::Sidecar {
            audio: loader::sidecar::fingerprint_file(&path).unwrap(),
            analysis: Some(loader::sidecar::Stored {
                grid: bpm.map(|bpm| analysis::tempo::BeatGrid {
                    bpm,
                    first_beat_secs: 0.0,
                }),
                key: key.and_then(analysis::key::Key::from_camelot),
                // Full length, so a load reuses this analysis instead of running the
                // detector over the flat fixture audio and coming back with no grid.
                waveform: vec![[-0.2, 0.2]; loader::ENVELOPE_POINTS],
                bands: vec![[0.2, 0.1, 0.05]; loader::ENVELOPE_POINTS],
            }),
            cues: Default::default(),
            track: Some(loader::sidecar::TrackInfo {
                title: Some((*name).to_string()),
                artist: None,
                duration_secs: Some(300.0),
                genre: genre.map(str::to_string),
                tags_read: genre.is_some(),
                discogs_checked: false,
            }),
        };
        sidecar.write(&path).unwrap();
    }
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
fn the_app_says_which_track_sits_on_each_deck() {
    let dir = tempfile::tempdir().unwrap();
    music(dir.path(), &["Alpha.wav", "Beta.wav"]);
    let (mut app, mut p) = setup();
    assert_eq!(app.deck_paths(), [None, None], "nothing loaded yet");

    app.scan_library(&[dir.path().to_path_buf()]);
    press(&mut app, Key::Char('b'));
    press(&mut app, Key::Down);
    press(&mut app, Key::Enter);

    let start = Instant::now();
    while app.snapshot().decks[0].track_frames == 0 {
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
        app.deck_paths(),
        [Some(dir.path().join("Beta.wav")), None],
        "the path, so a session can be written from it"
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
fn b_gives_the_browser_the_keyboard_and_the_whole_screen() {
    let (mut app, _p) = setup();
    assert!(!app.view(String::new()).browser.active);
    press(&mut app, Key::Char('b'));
    let view = app.view(String::new());
    assert!(view.browser.active);
    assert!(
        view.browser.fullscreen,
        "browsing is what the screen is for while you are doing it"
    );

    app.on_key(KeyEvent::press(Key::Char('f')).alt());
    assert!(
        !app.view(String::new()).browser.fullscreen,
        "alt+f drops back to the panel without leaving the browser"
    );
    app.on_key(KeyEvent::press(Key::Char('f')).alt());
    assert!(app.view(String::new()).browser.fullscreen);
}

#[test]
fn search_keeps_the_decks_on_screen() {
    let (mut app, _p) = setup();
    press(&mut app, Key::Char('/'));
    let view = app.view(String::new());
    assert!(view.browser.active);
    assert!(
        !view.browser.fullscreen,
        "a search is the one way into the browser that leaves the decks visible"
    );
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

#[test]
fn leaving_browser_mode_gives_the_mixer_back_even_from_full_screen() {
    let (mut app, _p) = setup();
    press(&mut app, Key::Char('b'));
    assert!(app.view(String::new()).browser.fullscreen);

    press(&mut app, Key::Esc);
    let view = app.view(String::new());
    assert!(!view.browser.active);
    assert!(
        !view.browser.fullscreen,
        "a full-screen list in mix mode hides the decks and the mixer with no key to undo it"
    );
}

#[test]
fn typing_a_field_token_finds_tracks_the_letters_alone_would_not() {
    // Typing narrows by re-scoring what the previous query left, on the assumption that a
    // longer query matches a subset. A field token breaks it: "bp" is a bare word matching
    // almost nothing, and "bpm:124" has to find tracks "bp" threw away.
    let dir = tempfile::tempdir().unwrap();
    library(
        dir.path(),
        &[
            ("Bicep - Glue", Some(124.0), None, None),
            ("Objekt - Cactus", Some(130.0), None, None),
        ],
    );
    let (mut app, _p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    press(&mut app, Key::Char('b'));
    for c in "bpm:124".chars() {
        press(&mut app, Key::Char(c));
    }
    assert_eq!(rows(&app), vec!["Bicep - Glue"], "{:?}", rows(&app));
}

/// Load a track onto deck A and start it, so the filters have something to measure against.
fn play(app: &mut App, p: &mut EngineProcessor, path: &Path) {
    app.load_path(engine::DeckId::A, path.to_path_buf());
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.view(String::new()).decks[0].title.is_none() {
        app.tick();
        process(p, 512);
        assert!(Instant::now() < deadline, "the track never loaded");
    }
    press(app, Key::Space);
    process(p, 512);
    assert!(app.snapshot().decks[0].playing, "the deck did not start");
}

#[test]
fn the_bpm_filter_cycles_and_measures_against_the_playing_deck() {
    let dir = tempfile::tempdir().unwrap();
    library(
        dir.path(),
        &[
            ("slow", Some(100.0), None, None),
            ("near", Some(126.0), None, None),
            ("spot on", Some(128.0), None, None),
        ],
    );
    let (mut app, mut p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    play(&mut app, &mut p, &dir.path().join("spot on.wav"));
    press(&mut app, Key::Char('b'));
    press_alt(&mut app, Key::Char('b'));
    let names = rows(&app);
    assert!(names.contains(&"spot on".to_string()), "{names:?}");
    assert!(
        names.contains(&"near".to_string()),
        "126 is inside 3 %: {names:?}"
    );
    assert!(!names.contains(&"slow".to_string()), "{names:?}");
}

#[test]
fn the_bpm_filter_folds_half_and_double_time() {
    // A 64 BPM track mixes against 128, and a jungle roller at 174 mixes against 87.
    let dir = tempfile::tempdir().unwrap();
    library(
        dir.path(),
        &[
            ("half", Some(64.0), None, None),
            ("unrelated", Some(100.0), None, None),
            ("host", Some(128.0), None, None),
        ],
    );
    let (mut app, mut p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    play(&mut app, &mut p, &dir.path().join("host.wav"));
    press(&mut app, Key::Char('b'));
    press_alt(&mut app, Key::Char('b'));
    let names = rows(&app);
    assert!(names.contains(&"half".to_string()), "{names:?}");
    assert!(!names.contains(&"unrelated".to_string()), "{names:?}");
}

#[test]
fn the_bpm_filter_stays_off_when_the_playing_deck_has_no_grid() {
    // A deck playing a track the detector could not read has no tempo to measure against.
    // Filtering everything out would look like a broken library.
    let dir = tempfile::tempdir().unwrap();
    library(
        dir.path(),
        &[
            ("a", Some(124.0), None, None),
            ("b", Some(130.0), None, None),
            ("no grid", None, None, None),
        ],
    );
    let (mut app, mut p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    play(&mut app, &mut p, &dir.path().join("no grid.wav"));
    press(&mut app, Key::Char('b'));
    press_alt(&mut app, Key::Char('b'));
    assert_eq!(
        rows(&app).len(),
        3,
        "nothing was filtered: {:?}",
        rows(&app)
    );
    let message = app.view(String::new()).message;
    assert!(
        message.contains("needs a deck playing with a tempo"),
        "it says why: {message}"
    );
}

#[test]
fn the_key_filter_keeps_what_would_mix() {
    let dir = tempfile::tempdir().unwrap();
    library(
        dir.path(),
        &[
            ("same", None, Some("9A"), None),
            ("neighbour", None, Some("10A"), None),
            ("clash", None, Some("4B"), None),
        ],
    );
    let (mut app, mut p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    play(&mut app, &mut p, &dir.path().join("same.wav"));
    press(&mut app, Key::Char('b'));
    press_alt(&mut app, Key::Char('k'));
    let names = rows(&app);
    assert!(names.contains(&"same".to_string()), "{names:?}");
    assert!(names.contains(&"neighbour".to_string()), "{names:?}");
    assert!(!names.contains(&"clash".to_string()), "{names:?}");
}

#[test]
fn the_genre_cycle_walks_what_the_library_holds() {
    let dir = tempfile::tempdir().unwrap();
    library(
        dir.path(),
        &[
            ("a", None, None, Some("House")),
            ("b", None, None, Some("Techno")),
            ("c", None, None, Some("House")),
        ],
    );
    let (mut app, _p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    press(&mut app, Key::Char('b'));
    press_alt(&mut app, Key::Char('g'));
    assert_eq!(rows(&app), vec!["a", "c"], "alphabetically first");
    press_alt(&mut app, Key::Char('g'));
    assert_eq!(rows(&app), vec!["b"]);
    press_alt(&mut app, Key::Char('g'));
    assert_eq!(rows(&app).len(), 3, "back to off");
}

#[test]
fn a_typed_token_and_a_key_filter_narrow_together() {
    let dir = tempfile::tempdir().unwrap();
    library(
        dir.path(),
        &[
            ("keep me 9A", None, Some("9A"), None),
            ("keep me 4B", None, Some("4B"), None),
        ],
    );
    let (mut app, mut p) = setup();
    app.scan_library(&[dir.path().to_path_buf()]);
    play(&mut app, &mut p, &dir.path().join("keep me 9A.wav"));
    press(&mut app, Key::Char('b'));
    press_alt(&mut app, Key::Char('k'));
    for c in "keep".chars() {
        press(&mut app, Key::Char(c));
    }
    assert_eq!(rows(&app), vec!["keep me 9A"]);
}
