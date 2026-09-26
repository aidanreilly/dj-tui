//! Applying input actions to the engine.

use dj_tui::apply::{apply, Controls};
use engine::{DeckId::*, Engine, Track};
use input::{Action, Dir};
use std::sync::Arc;

fn loaded() -> Engine {
    let mut e = Engine::new();
    let t = Arc::new(Track::from_interleaved(vec![0.0; 2000], 1000));
    e.deck_mut(A).load(t.clone());
    e.deck_mut(B).load(t);
    e
}

fn ctl() -> Controls {
    Controls::new(8)
}

#[test]
fn play_pause_and_cue_reach_the_right_deck() {
    let mut e = loaded();
    let c = ctl();
    assert!(apply(&mut e, &c, Action::PlayPause(B)));
    assert!(e.deck(B).is_playing() && !e.deck(A).is_playing());
    apply(&mut e, &c, Action::CuePress(B));
    assert!(!e.deck(B).is_playing());
    apply(&mut e, &c, Action::CuePress(A));
    assert!(e.deck(A).is_playing());
    apply(&mut e, &c, Action::CueRelease(A));
    assert!(!e.deck(A).is_playing());
}

#[test]
fn hot_cues_set_and_clear() {
    let mut e = loaded();
    let c = ctl();
    e.deck_mut(A).seek(100.0);
    apply(&mut e, &c, Action::HotCue(A, 3));
    assert_eq!(e.deck(A).hot_cue_position(3), Some(100.0));
    apply(&mut e, &c, Action::ClearHotCue(A, 3));
    assert_eq!(e.deck(A).hot_cue_position(3), None);
}

#[test]
fn crossfader_steps_and_snaps() {
    let mut e = loaded();
    let c = ctl();
    apply(&mut e, &c, Action::Crossfader(Dir::Up, false));
    assert!((e.crossfader() - c.crossfader_step).abs() < 1e-6);
    apply(&mut e, &c, Action::Crossfader(Dir::Down, true));
    assert_eq!(e.crossfader(), -1.0);
    apply(&mut e, &c, Action::Crossfader(Dir::Down, false));
    assert_eq!(e.crossfader(), -1.0);
}

#[test]
fn channel_fader_steps_within_range() {
    let mut e = loaded();
    let c = ctl();
    apply(&mut e, &c, Action::Fader(A, Dir::Up));
    assert_eq!(e.channel_fader(A), 1.0);
    apply(&mut e, &c, Action::Fader(A, Dir::Down));
    assert!((e.channel_fader(A) - (1.0 - c.fader_step)).abs() < 1e-6);
}

#[test]
fn headphone_cue_toggles() {
    let mut e = loaded();
    let c = ctl();
    apply(&mut e, &c, Action::HeadphoneCue(B));
    assert!(e.headphone_cue(B));
    apply(&mut e, &c, Action::HeadphoneCue(B));
    assert!(!e.headphone_cue(B));
}

#[test]
fn tempo_steps_and_is_limited_to_the_configured_range() {
    let mut e = loaded();
    let c = ctl();
    apply(&mut e, &c, Action::Tempo(A, Dir::Up, false));
    assert!((e.deck(A).rate() - 1.005).abs() < 1e-9);
    apply(&mut e, &c, Action::Tempo(A, Dir::Down, true));
    assert!((e.deck(A).rate() - 1.0045).abs() < 1e-9);
    for _ in 0..100 {
        apply(&mut e, &c, Action::Tempo(A, Dir::Up, false));
    }
    assert!((e.deck(A).rate() - 1.08).abs() < 1e-9);
}

#[test]
fn seek_tenth_moves_proportionally() {
    let mut e = loaded();
    let c = ctl();
    apply(&mut e, &c, Action::SeekTenth(A, 5));
    assert_eq!(e.deck(A).position(), 500.0);
    apply(&mut e, &c, Action::SeekTenth(A, 0));
    assert_eq!(e.deck(A).position(), 0.0);
}

#[test]
fn actions_the_engine_cannot_do_yet_report_unhandled() {
    let mut e = loaded();
    assert!(!apply(&mut e, &ctl(), Action::FxToggle(A)));
}
