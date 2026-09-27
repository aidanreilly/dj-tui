use dj_tui::clock::NullClock;
use dj_tui::demo::{click_track, peak_envelope};
use dj_tui::view::{screen_view, DeckMeta};
use engine::{channel, Command, DeckId, Engine, Snapshot};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn click_track_has_a_click_on_every_beat() {
    let t = click_track(120.0, 2.0, 1000);
    assert_eq!(t.frames(), 2000);
    // 120 BPM at 1 kHz: beats every 500 frames.
    for beat in [0usize, 500, 1000, 1500] {
        assert!(t.frame_at(beat as f64).0.abs() > 0.5, "no click at {beat}");
    }
    assert!(t.frame_at(250.0).0.abs() < 0.01);
}

#[test]
fn peak_envelope_is_normalised_to_one() {
    let t = click_track(120.0, 2.0, 1000);
    let env = peak_envelope(&t, 40);
    assert_eq!(env.len(), 40);
    let max = env.iter().copied().fold(0.0, f32::max);
    assert!((max - 1.0).abs() < 1e-6);
    assert!(env.contains(&0.0));
}

#[test]
fn view_reports_times_in_seconds_and_focus() {
    let mut snap = Snapshot::default();
    snap.decks[1].track_frames = 10_000;
    snap.decks[1].position = 2500.0;
    snap.decks[1].hot_cues[1] = Some(100.0);
    snap.decks[1].playing = true;
    snap.faders = [0.5, 1.0];
    let metas = [
        DeckMeta::default(),
        DeckMeta {
            title: Some("Demo".into()),
            bpm: Some(124.0),
            key: None,
            loading: false,
            waveform: vec![[0.0, 0.5]],
            bands: vec![],
            grid: None,
        },
    ];
    let v = screen_view(&snap, 1000, DeckId::B, &metas, "ok".into());
    let b = &v.decks[1];
    assert!(b.focused && !v.decks[0].focused);
    assert_eq!(b.position_secs, 2.5);
    assert_eq!(b.duration_secs, 10.0);
    assert!(b.hot_cues[1] && !b.hot_cues[0]);
    assert!(b.playing);
    assert_eq!(b.title.as_deref(), Some("Demo"));
    assert!(v.decks[0].title.is_none(), "no track loaded on A");
    assert_eq!(v.mixer.faders, [0.5, 1.0]);
}

fn playing_processor(rate: u32) -> (engine::EngineHandle, engine::EngineProcessor) {
    let (mut h, p) = channel(Engine::new(), 8);
    h.send(Command::Load(
        DeckId::A,
        Arc::new(click_track(120.0, 30.0, rate)),
    ))
    .unwrap();
    h.send(Command::PlayPause(DeckId::A)).unwrap();
    (h, p)
}

#[test]
fn null_clock_advances_playing_decks_by_elapsed_time() {
    let (h, mut p) = playing_processor(48_000);
    let mut clock = NullClock::new(48_000);
    clock.advance(&mut p, Duration::from_millis(250));
    assert_eq!(h.snapshot().decks[0].position, 12_000.0);
    clock.advance(&mut p, Duration::from_secs(1));
    assert_eq!(h.snapshot().decks[0].position, 60_000.0);
}

#[test]
fn null_clock_carries_fractional_frames() {
    let (h, mut p) = playing_processor(1000);
    let mut clock = NullClock::new(1000);
    for _ in 0..4 {
        clock.advance(&mut p, Duration::from_micros(2500));
    }
    assert_eq!(h.snapshot().decks[0].position, 10.0);
}
