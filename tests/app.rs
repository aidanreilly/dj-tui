//! The UI-side application state: keys in, commands out, loads in the background.

mod common;
use common::{stereo, write_wav, Fmt};
use dj_tui::app::App;
use dj_tui::config::Config;
use engine::{channel, DeckId, Engine, EngineProcessor};
use input::{Key, KeyEvent};
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

fn wait_for_load(app: &mut App, p: &mut EngineProcessor, deck: DeckId) {
    let start = Instant::now();
    while app.snapshot().decks[deck.index()].track_frames == 0 {
        app.tick();
        process(p, 16);
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "load timed out: {}",
            app.message()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn loading_a_file_puts_it_on_the_deck_with_its_title() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Warehouse Tool.wav");
    write_wav(&path, &stereo(&[0.3; 4800]), 2, RATE, Fmt::Pcm16);
    let (mut app, mut p) = setup();
    app.load_path(DeckId::B, path);
    wait_for_load(&mut app, &mut p, DeckId::B);
    let view = app.view(String::new());
    assert_eq!(view.decks[1].title.as_deref(), Some("Warehouse Tool"));
    assert!((view.decks[1].duration_secs - 0.1).abs() < 1e-9);
    assert!(
        app.message().contains("Warehouse Tool"),
        "{}",
        app.message()
    );
    assert_eq!(view.message, app.message());
}

#[test]
fn failed_loads_show_a_message_and_leave_the_deck_empty() {
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, "/definitely/missing.flac".into());
    let start = Instant::now();
    while !app.message().contains("missing.flac") {
        app.tick();
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
    }
    process(&mut p, 16);
    assert_eq!(app.snapshot().decks[0].track_frames, 0);
}

#[test]
fn keys_drive_the_focused_deck() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    write_wav(&path, &stereo(&[0.3; 48_000]), 2, RATE, Fmt::Pcm16);
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, path);
    wait_for_load(&mut app, &mut p, DeckId::A);

    assert!(!app.on_key(KeyEvent::press(Key::Space)));
    process(&mut p, 100);
    assert!(app.snapshot().decks[0].playing);

    app.on_key(KeyEvent::press(Key::Tab));
    app.on_key(KeyEvent::press(Key::Space));
    process(&mut p, 1);
    assert!(!app.snapshot().decks[1].playing, "deck B is empty");
    assert!(app.view(String::new()).decks[1].focused);
}

#[test]
fn ctrl_q_asks_to_quit() {
    let (mut app, _p) = setup();
    assert!(app.on_key(KeyEvent::press(Key::Char('q')).ctrl()));
}

#[test]
fn configured_crossfader_curve_is_sent_at_start() {
    let (handle, mut p) = channel(Engine::new(), 64);
    let config = Config::from_toml("[mixer]\ncrossfader_curve = \"cut\"").unwrap();
    let mut app = App::new(handle, &config, RATE);
    process(&mut p, 1);
    app.tick();
    assert_eq!(
        app.snapshot().crossfader_curve,
        engine::CrossfaderCurve::Cut
    );
}

#[test]
fn meters_show_level_and_fall_back_gradually() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loud.wav");
    write_wav(&path, &stereo(&[0.5; 48_000]), 2, RATE, Fmt::Pcm16);
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, path);
    wait_for_load(&mut app, &mut p, DeckId::A);
    app.on_key(KeyEvent::press(Key::Space));
    process(&mut p, 4800);
    app.tick();
    let level = app.view(String::new()).mixer.strips[0].meter;
    assert!((level - 0.5).abs() < 0.02, "{level}");
    app.on_key(KeyEvent::press(Key::Space));
    process(&mut p, 480);
    app.tick();
    let falling = app.view(String::new()).mixer.strips[0].meter;
    assert!(falling > 0.2 && falling < level, "{falling}");
    for _ in 0..200 {
        process(&mut p, 480);
        app.tick();
    }
    assert!(app.view(String::new()).mixer.strips[0].meter < 0.01);
}

#[test]
fn waveform_mode_starts_from_config_and_w_cycles_it() {
    use tui::pixel::WaveformMode;
    let (handle, _p) = channel(Engine::new(), 64);
    let config = Config::from_toml("[ui]\nwaveform_mode = \"rgb\"").unwrap();
    let mut app = App::new(handle, &config, RATE);
    assert_eq!(
        app.view(String::new()).decks[0].waveform_mode,
        WaveformMode::Rgb
    );
    app.on_key(KeyEvent::press(Key::Char('W')));
    let v = app.view(String::new());
    assert_eq!(v.decks[0].waveform_mode, WaveformMode::Blue);
    assert_eq!(v.decks[1].waveform_mode, WaveformMode::Blue);
    assert!(app.message().contains("Blue"), "{}", app.message());
}

#[test]
fn loaded_tracks_carry_band_data_to_the_view() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bands.wav");
    write_wav(&path, &stereo(&[0.3; 9600]), 2, RATE, Fmt::Pcm16);
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, path);
    wait_for_load(&mut app, &mut p, DeckId::A);
    assert_eq!(
        app.view(String::new()).decks[0].bands.len(),
        loader::ENVELOPE_POINTS
    );
}
