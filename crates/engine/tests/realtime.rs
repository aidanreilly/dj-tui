//! Lock-free command queue into the audio thread, state snapshots back out,
//! and disposal of replaced tracks off the audio thread.

use engine::{channel, Command, CrossfaderCurve, DeckId::*, Engine, Track};
use std::sync::Arc;

fn track(frames: usize) -> Arc<Track> {
    Arc::new(Track::from_interleaved(vec![0.1; frames * 2], 48_000))
}

fn run(p: &mut engine::EngineProcessor, frames: usize) {
    let mut m = vec![0.0; frames * 2];
    let mut c = vec![0.0; frames * 2];
    p.process(&mut m, &mut c);
}

#[test]
fn commands_take_effect_on_the_next_process_call() {
    let (mut h, mut p) = channel(Engine::new(), 16);
    h.send(Command::Load(A, track(1000))).unwrap();
    h.send(Command::PlayPause(A)).unwrap();
    assert!(!h.snapshot().decks[0].playing);
    run(&mut p, 10);
    let s = h.snapshot();
    assert!(s.decks[0].playing);
    assert_eq!(s.decks[0].position, 10.0);
    assert!(!s.decks[1].playing);
}

#[test]
fn snapshot_reflects_cues_and_mixer_state() {
    let (mut h, mut p) = channel(Engine::new(), 16);
    for cmd in [
        Command::Load(B, track(1000)),
        Command::Seek(B, 250.0),
        Command::HotCue(B, 2),
        Command::SetCrossfader(0.5),
        Command::SetChannelFader(A, 0.25),
        Command::SetHeadphoneCue(B, true),
        Command::SetRate(B, 1.02),
        Command::SetCrossfaderCurve(CrossfaderCurve::Cut),
        Command::SetCueMix(0.3),
    ] {
        h.send(cmd).unwrap();
    }
    run(&mut p, 1);
    let s = h.snapshot();
    assert_eq!(s.decks[1].hot_cues[2], Some(250.0));
    assert_eq!(s.decks[1].hot_cues[0], None);
    assert_eq!(s.decks[1].position, 250.0);
    assert!((s.decks[1].rate - 1.02).abs() < 1e-12);
    assert_eq!(s.crossfader, 0.5);
    assert_eq!(s.faders, [0.25, 1.0]);
    assert_eq!(s.headphone_cue, [false, true]);
    assert_eq!(s.crossfader_curve, CrossfaderCurve::Cut);
    assert_eq!(s.frames_processed, 1);
}

#[test]
fn replaced_tracks_are_handed_back_instead_of_dropped_on_the_audio_thread() {
    let (mut h, mut p) = channel(Engine::new(), 16);
    let first = track(100);
    h.send(Command::Load(A, first.clone())).unwrap();
    run(&mut p, 1);
    h.send(Command::Load(A, track(100))).unwrap();
    run(&mut p, 1);
    // Still alive: held by the garbage queue, not freed inside process().
    assert_eq!(Arc::strong_count(&first), 2);
    assert_eq!(h.collect_garbage(), 1);
    assert_eq!(Arc::strong_count(&first), 1);
}

#[test]
fn full_queue_returns_the_command() {
    let (mut h, _p) = channel(Engine::new(), 2);
    h.send(Command::PlayPause(A)).unwrap();
    h.send(Command::PlayPause(A)).unwrap();
    assert!(matches!(
        h.send(Command::PlayPause(B)),
        Err(Command::PlayPause(B))
    ));
}

#[test]
fn snapshot_reports_track_length() {
    let (mut h, mut p) = channel(Engine::new(), 4);
    h.send(Command::Load(A, track(777))).unwrap();
    run(&mut p, 1);
    let s = h.snapshot();
    assert_eq!(s.decks[0].track_frames, 777);
    assert_eq!(s.decks[1].track_frames, 0);
}
