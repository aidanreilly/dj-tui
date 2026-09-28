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

/// A click every beat, which the tempo detector locks onto.
fn click_track(bpm: f64, secs: f64) -> Vec<f32> {
    let beat = 60.0 / bpm as f32;
    (0..(secs as f32 * RATE as f32) as usize)
        .map(|i| {
            let tb = (i as f32 / RATE as f32) % beat;
            if tb < 0.02 {
                (-tb / 0.004).exp() * (std::f32::consts::TAU * 1500.0 * tb).sin()
            } else {
                0.0
            }
        })
        .collect()
}

#[test]
fn the_loop_key_loops_four_beats_and_the_length_keys_change_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loop.wav");
    write_wav(
        &path,
        &stereo(&click_track(120.0, 20.0)),
        2,
        RATE,
        Fmt::Float32,
    );
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, path);
    wait_for_load(&mut app, &mut p, DeckId::A);

    app.on_key(KeyEvent::press(Key::Char('l')));
    process(&mut p, 16);
    let (start, end) = app.snapshot().decks[0]
        .loop_span
        .expect("a loop is running");
    let beats = (end - start) / (60.0 / 120.0 * RATE as f64);
    assert!((beats - 4.0).abs() < 0.05, "four beats long, got {beats}");
    assert!(app.message().contains("4 beats"), "{}", app.message());
    assert_eq!(
        app.view(String::new()).decks[0].loop_secs,
        Some((start / RATE as f64, end / RATE as f64)),
        "the view carries the loop so the waveform can draw it"
    );

    app.on_key(KeyEvent::press(Key::Char('[')));
    process(&mut p, 16);
    let (halved_start, halved_end) = app.snapshot().decks[0].loop_span.unwrap();
    assert_eq!(halved_start, start, "halving keeps the in point");
    assert!(
        ((halved_end - halved_start) / (end - start) - 0.5).abs() < 1e-6,
        "half as long"
    );
    assert!(app.message().contains("2 beats"), "{}", app.message());

    app.on_key(KeyEvent::press(Key::Char('l')));
    process(&mut p, 16);
    assert_eq!(app.snapshot().decks[0].loop_span, None);
    assert!(app.view(String::new()).decks[0].loop_secs.is_none());
    assert!(app.message().contains("Loop off"), "{}", app.message());
}

#[test]
fn marking_a_loop_by_hand_reports_both_ends_and_shows_the_in_point() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("manual.wav");
    write_wav(&path, &stereo(&[0.3; 480_000]), 2, RATE, Fmt::Pcm16);
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, path);
    wait_for_load(&mut app, &mut p, DeckId::A);

    app.on_key(KeyEvent::press(Key::Char('i')));
    assert!(app.message().contains("Loop in"), "{}", app.message());
    assert_eq!(
        app.view(String::new()).decks[0].loop_in_secs,
        Some(0.0),
        "the waiting in point shows under the waveform"
    );

    app.seek_to_fraction(DeckId::A, 0.5);
    process(&mut p, 16);
    app.on_key(KeyEvent::press(Key::Char('I')));
    process(&mut p, 16);
    let (start, end) = app.snapshot().decks[0]
        .loop_span
        .expect("a loop is running");
    assert_eq!(start, 0.0);
    assert!((end - 240_000.0).abs() < 1.0, "out point at the playhead");
    assert!(app.message().contains("Loop"), "{}", app.message());
    assert!(app.view(String::new()).decks[0].loop_in_secs.is_none());
}

#[test]
fn the_loop_out_key_alone_says_what_is_missing() {
    let (mut app, _p) = setup();
    app.on_key(KeyEvent::press(Key::Char('I')));
    assert!(
        app.message().contains("no loop in point yet"),
        "{}",
        app.message()
    );
}

#[test]
fn loading_a_track_forgets_the_loop_in_point_from_the_last_one() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.wav");
    let second = dir.path().join("second.wav");
    write_wav(&first, &stereo(&[0.3; 96_000]), 2, RATE, Fmt::Pcm16);
    write_wav(&second, &stereo(&[0.2; 96_000]), 2, RATE, Fmt::Pcm16);
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, first);
    wait_for_load(&mut app, &mut p, DeckId::A);
    app.on_key(KeyEvent::press(Key::Char('i')));
    assert!(app.view(String::new()).decks[0].loop_in_secs.is_some());

    app.load_path(DeckId::A, second);
    let start = Instant::now();
    while app.view(String::new()).decks[0].loop_in_secs.is_some() {
        app.tick();
        process(&mut p, 16);
        assert!(start.elapsed() < Duration::from_secs(10), "load timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_loop_left_running_is_saved_beside_the_file_and_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("looped.wav");
    write_wav(&path, &stereo(&[0.3; 480_000]), 2, RATE, Fmt::Pcm16);
    {
        let (mut app, mut p) = setup();
        app.load_path(DeckId::A, path.clone());
        wait_for_load(&mut app, &mut p, DeckId::A);
        app.on_key(KeyEvent::press(Key::Char('i')));
        app.seek_to_fraction(DeckId::A, 0.25);
        process(&mut p, 16);
        app.on_key(KeyEvent::press(Key::Char('I')));
        process(&mut p, 16);
        assert!(app.snapshot().decks[0].loop_span.is_some());
        // Saving happens on the next tick, from the background thread.
        let start = Instant::now();
        while loader::sidecar::Sidecar::read(&path)
            .and_then(|s| s.cues.loop_secs)
            .is_none()
        {
            app.tick();
            process(&mut p, 16);
            assert!(start.elapsed() < Duration::from_secs(10), "save timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, path);
    wait_for_load(&mut app, &mut p, DeckId::A);
    process(&mut p, 16);
    let (start, end) = app.snapshot().decks[0]
        .loop_span
        .expect("the loop came back with the track");
    assert_eq!(start, 0.0);
    assert!((end - 120_000.0).abs() < 1.0, "quarter of the way in");
}

#[test]
fn sync_pulls_the_focused_deck_to_the_other_ones_tempo() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.wav"), dir.path().join("b.wav"));
    write_wav(
        &a,
        &stereo(&click_track(120.0, 20.0)),
        2,
        RATE,
        Fmt::Float32,
    );
    write_wav(
        &b,
        &stereo(&click_track(126.0, 20.0)),
        2,
        RATE,
        Fmt::Float32,
    );
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, a);
    app.load_path(DeckId::B, b);
    wait_for_load(&mut app, &mut p, DeckId::A);
    wait_for_load(&mut app, &mut p, DeckId::B);

    let before = app.view(String::new()).decks[0].bpm.unwrap();
    assert!(
        (before - 120.0).abs() < 0.5,
        "deck A starts at its own tempo"
    );
    app.on_key(KeyEvent::press(Key::Char('s')));
    process(&mut p, 16);
    let after = app.view(String::new()).decks[0].bpm.unwrap();
    assert!(
        (after - 126.0).abs() < 0.5,
        "deck A now runs at deck B's tempo: {after}"
    );
    assert!(app.message().contains("Sync"), "{}", app.message());
}

#[test]
fn sync_without_a_grid_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("flat.wav");
    // Steady tone: nothing for the tempo detector to lock onto.
    write_wav(&path, &stereo(&[0.3; 96_000]), 2, RATE, Fmt::Pcm16);
    let (mut app, mut p) = setup();
    app.load_path(DeckId::A, path);
    wait_for_load(&mut app, &mut p, DeckId::A);
    app.on_key(KeyEvent::press(Key::Char('s')));
    assert!(
        app.message().contains("grid"),
        "it says why nothing happened: {}",
        app.message()
    );
}

#[test]
fn the_effect_knobs_report_what_they_do_in_the_unit_they_are_turning() {
    let (mut app, _p) = setup();
    app.on_key(KeyEvent::press(Key::Char('P')));
    assert!(app.message().contains("Echo time"), "{}", app.message());
    app.on_key(KeyEvent::press(Key::Char('F')));
    app.on_key(KeyEvent::press(Key::Char('d')));
    assert!(app.message().contains("Flanger depth"), "{}", app.message());
}

#[test]
fn a_controller_drives_the_same_actions_the_keys_do() {
    use input::Action;
    use midi::Control;
    let (mut app, mut p) = setup();
    assert!(!app.on_action(Action::CycleWaveformMode));
    assert!(app.message().contains("Waveform"), "{}", app.message());

    app.set_control(Control::Fader(DeckId::B), 0.25);
    process(&mut p, 16);
    assert!((app.snapshot().faders[1] - 0.25).abs() < 1e-6);
    assert_eq!(app.control_value(Control::Fader(DeckId::B)), 0.25);

    assert!(app.on_action(Action::Quit), "quit still quits");
}

#[test]
fn the_question_mark_shows_the_key_list_and_any_key_puts_it_away() {
    let (mut app, _p) = setup();
    assert!(!app.view(String::new()).help);
    app.on_key(KeyEvent::press(Key::Char('?')));
    assert!(app.view(String::new()).help, "the list is up");
    app.on_key(KeyEvent::press(Key::Char('?')));
    assert!(
        !app.view(String::new()).help,
        "and the same key puts it away"
    );

    app.on_key(KeyEvent::press(Key::Char('?')));
    app.on_key(KeyEvent::press(Key::Space));
    assert!(
        !app.view(String::new()).help,
        "so does getting on with something else"
    );
}

#[test]
fn the_headphone_mix_key_reports_both_sides_of_the_blend() {
    let (mut app, _p) = setup();
    app.on_key(KeyEvent::press(Key::Char('H')));
    assert!(
        app.message().contains("40% cue") && app.message().contains("60% master"),
        "{}",
        app.message()
    );
    assert!((app.view(String::new()).mixer.cue_mix - 0.6).abs() < 1e-6);
}

#[test]
fn what_went_wrong_is_kept_for_the_log() {
    let (mut app, mut p) = setup();
    assert!(app.take_log().is_empty(), "nothing has happened yet");

    app.load_path(DeckId::A, "/definitely/missing.flac".into());
    let start = Instant::now();
    while app.take_log().is_empty() {
        app.tick();
        process(&mut p, 16);
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "no log line arrived"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fine.wav");
    write_wav(&path, &stereo(&[0.3; 4_800]), 2, RATE, Fmt::Pcm16);
    app.load_path(DeckId::A, path);
    wait_for_load(&mut app, &mut p, DeckId::A);
    assert!(
        app.take_log().is_empty(),
        "a load that worked is not worth a line"
    );
}

#[test]
fn key_lock_reports_itself_and_shows_in_the_deck_view() {
    let (mut app, _p) = setup();
    app.on_key(KeyEvent::press(Key::Char('k')));
    assert!(app.message().contains("Key lock on"), "{}", app.message());
    assert!(app.view(String::new()).decks[0].key_lock);
    app.on_key(KeyEvent::press(Key::Char('k')));
    assert!(app.message().contains("Key lock off"), "{}", app.message());
    assert!(!app.view(String::new()).decks[0].key_lock);
}

#[test]
fn quantize_reports_itself_and_shows_in_the_deck_view() {
    let (mut app, _p) = setup();
    app.on_key(KeyEvent::press(Key::Char('q')));
    assert!(app.message().contains("Quantize on"), "{}", app.message());
    assert!(app.view(String::new()).decks[0].quantize);
    app.on_key(KeyEvent::press(Key::Char('q')));
    assert!(app.message().contains("Quantize off"), "{}", app.message());
    assert!(!app.view(String::new()).decks[0].quantize);
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

mod fades {
    use super::*;

    /// Advance the fades and let the engine apply what they sent. `mixer.crossfader` and
    /// `mixer.faders` come off the engine snapshot, so without the processor running they
    /// never move and an assertion on them cannot fail.
    fn settle(app: &mut App, p: &mut EngineProcessor, dt: f64) {
        app.tick_fades(dt);
        process(p, 16);
    }

    #[test]
    fn a_crossfader_fade_moves_it_over_several_frames_and_lands_on_the_end() {
        let (mut app, mut p) = setup();
        app.on_key(KeyEvent::press(Key::Char('x')));
        process(&mut p, 16);
        assert_eq!(app.view(String::new()).mixer.crossfader, 0.0);
        app.on_key(KeyEvent::press(Key::Right).alt());
        assert_eq!(
            app.view(String::new()).mixer.crossfader_fade_target,
            Some(1.0),
            "the marker shows where it is going"
        );

        settle(&mut app, &mut p, 0.1);
        let part = app.view(String::new()).mixer.crossfader;
        assert!(part > 0.0 && part < 1.0, "on its way: {part}");

        settle(&mut app, &mut p, 60.0);
        let v = app.view(String::new());
        assert_eq!(v.mixer.crossfader, 1.0);
        assert_eq!(v.mixer.crossfader_fade_target, None, "it finished");
    }

    #[test]
    fn a_manual_move_takes_the_control_back_from_its_fade() {
        let (mut app, mut p) = setup();
        app.on_key(KeyEvent::press(Key::Right).alt());
        settle(&mut app, &mut p, 0.05);
        app.on_key(KeyEvent::press(Key::Left));
        process(&mut p, 16);
        let before = app.view(String::new()).mixer.crossfader;
        assert!(before < 0.0, "the step moved it left: {before}");
        settle(&mut app, &mut p, 10.0);
        assert_eq!(app.view(String::new()).mixer.crossfader, before);
    }

    #[test]
    fn a_midi_knob_takes_the_control_back_from_its_fade() {
        let (mut app, mut p) = setup();
        app.on_key(KeyEvent::press(Key::Right).alt());
        settle(&mut app, &mut p, 0.05);
        app.set_control(midi::Control::Crossfader, 0.25);
        settle(&mut app, &mut p, 10.0);
        let v = app.view(String::new()).mixer.crossfader;
        assert!((v - -0.5).abs() < 1e-5, "the knob won: {v}");
    }

    /// Review Focus 4.
    #[test]
    fn esc_with_nothing_running_does_nothing() {
        let (mut app, mut p) = setup();
        let before = app.view(String::new()).mixer.crossfader;
        app.on_key(KeyEvent::press(Key::Esc));
        settle(&mut app, &mut p, 1.0);
        assert_eq!(app.view(String::new()).mixer.crossfader, before);
        assert_eq!(app.message(), "", "and says nothing about it");
    }

    #[test]
    fn esc_stops_every_fade_where_it_stands() {
        let (mut app, mut p) = setup();
        app.on_key(KeyEvent::press(Key::Right).alt());
        app.on_key(KeyEvent::press(Key::Down).alt());
        settle(&mut app, &mut p, 0.1);
        app.on_key(KeyEvent::press(Key::Esc));
        let held = app.view(String::new());
        assert!(held.mixer.crossfader > 0.0, "the crossfader had moved");
        assert!(held.mixer.faders[0] < 1.0, "and so had the fader");
        settle(&mut app, &mut p, 10.0);
        let after = app.view(String::new());
        assert_eq!(after.mixer.crossfader, held.mixer.crossfader);
        assert_eq!(after.mixer.faders[0], held.mixer.faders[0]);
        assert!(app.message().contains("Fades"), "{}", app.message());
    }

    #[test]
    fn the_fade_length_is_on_screen_and_scales_with_the_braces() {
        let (mut app, _p) = setup();
        let start = app.view(String::new()).mixer.fade_beats;
        assert_eq!(start, 8.0);
        app.on_key(KeyEvent::press(Key::Char('}')));
        assert_eq!(app.view(String::new()).mixer.fade_beats, 16.0);
        app.on_key(KeyEvent::press(Key::Char('{')));
        assert_eq!(app.view(String::new()).mixer.fade_beats, 8.0);
    }

    #[test]
    fn a_filter_sweep_runs_and_v_brings_it_back() {
        let (mut app, mut p) = setup();
        app.on_key(KeyEvent::press(Key::Char('O')).alt());
        settle(&mut app, &mut p, 60.0);
        assert!((app.view(String::new()).mixer.strips[0].filter - 1.0).abs() < 1e-6);
        app.on_key(KeyEvent::press(Key::Char('V')));
        settle(&mut app, &mut p, 60.0);
        assert_eq!(app.view(String::new()).mixer.strips[0].filter, 0.0);
    }
}
