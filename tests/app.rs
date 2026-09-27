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
fn seeking_to_a_fraction_moves_the_playhead_and_clamps_at_the_ends() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("seek.wav");
    write_wav(&path, &stereo(&[0.2; 9600]), 2, RATE, Fmt::Pcm16);
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, path);
    wait_for_load(&mut app, &mut p, DeckId::A);
    let frames = app.snapshot().decks[0].track_frames as f64;

    app.seek_to_fraction(DeckId::A, 0.5);
    process(&mut p, 16);
    assert!(
        (app.snapshot().decks[0].position - frames / 2.0).abs() < 1.0,
        "half way: {}",
        app.snapshot().decks[0].position
    );

    app.seek_to_fraction(DeckId::A, 2.0);
    process(&mut p, 16);
    assert!(
        (app.snapshot().decks[0].position - frames).abs() < 1.0,
        "past the end clamps to the end: {}",
        app.snapshot().decks[0].position
    );
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

#[test]
fn analysis_results_reach_the_deck_view() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("beats.wav");
    let beat = 60.0 / 124.0;
    let mono: Vec<f32> = (0..RATE as usize * 20)
        .map(|i| {
            let tb = (i as f32 / RATE as f32) % beat;
            if tb < 0.02 {
                (-tb / 0.004).exp() * (std::f32::consts::TAU * 1500.0 * tb).sin()
            } else {
                0.0
            }
        })
        .collect();
    write_wav(&path, &stereo(&mono), 2, RATE, Fmt::Float32);
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, path);
    wait_for_load(&mut app, &mut p, DeckId::A);
    let v = app.view(String::new());
    assert!(
        (v.decks[0].bpm.unwrap() - 124.0).abs() < 0.05,
        "{:?}",
        v.decks[0].bpm
    );
    assert!(v.decks[0].beat.is_some(), "bar and beat counter available");
}

#[test]
fn end_of_track_warning_flashes_in_the_last_stretch_while_playing() {
    use dj_tui::view::end_warning;
    assert!(end_warning(20.0, true, 30, 0.1));
    assert!(!end_warning(20.0, true, 30, 0.6), "off half of the flash");
    assert!(!end_warning(40.0, true, 30, 0.1), "too early");
    assert!(!end_warning(20.0, false, 30, 0.1), "paused");
    assert!(!end_warning(20.0, true, 0, 0.1), "disabled");
}

#[test]
fn phase_offset_is_b_relative_to_a_wrapped_to_half_a_beat() {
    use dj_tui::view::phase_offset;
    assert!((phase_offset(0.1, 0.35) - 0.25).abs() < 1e-9);
    assert!((phase_offset(0.9, 0.1) - 0.2).abs() < 1e-9);
    assert!((phase_offset(0.1, 0.9) + 0.2).abs() < 1e-9);
}

#[test]
fn cues_set_on_a_deck_are_saved_beside_the_file_and_come_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keep.wav");
    write_wav(&path, &stereo(&[0.3; 96_000]), 2, RATE, Fmt::Pcm16);
    let sidecar = loader::sidecar::sidecar_path(&path);
    {
        let (mut app, mut p) = setup();
        app.load_path(DeckId::A, path.clone());
        wait_for_load(&mut app, &mut p, DeckId::A);
        app.on_key(KeyEvent::press(Key::Space));
        process(&mut p, 4800);
        app.on_key(KeyEvent::press(Key::Char('2')));
        process(&mut p, 16);
        let start = Instant::now();
        loop {
            app.tick();
            let text = std::fs::read_to_string(&sidecar).unwrap_or_default();
            if text.contains("\"pad\":2") || text.contains("\"pad\": 2") {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "cue never saved: {text}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, path);
    wait_for_load(&mut app, &mut p, DeckId::A);
    process(&mut p, 16);
    let cue = app.snapshot().decks[0].hot_cues[1].expect("hot cue 2 restored");
    assert!((cue - 4800.0).abs() < 1.0, "{cue}");
}
