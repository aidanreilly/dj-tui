//! Turning input actions into engine commands. The UI owns the absolute values of mixer
//! controls and tempo, so repeated key presses between audio callbacks never get lost.

use dj_tui::apply::{apply, ControlState, Controls};
use engine::{channel, Command, DeckId::*, Engine, Snapshot, Track};
use input::{Action, Dir};
use std::sync::Arc;

fn ctl() -> Controls {
    Controls::new(8)
}

fn snap_with_frames(a: usize, b: usize) -> Snapshot {
    let mut s = Snapshot::default();
    s.decks[0].track_frames = a;
    s.decks[1].track_frames = b;
    s
}

fn run(state: &mut ControlState, snap: &Snapshot, action: Action) -> Option<Command> {
    apply(state, &ctl(), snap, action)
}

#[test]
fn transport_actions_become_matching_commands() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    assert!(matches!(
        run(&mut st, &s, Action::PlayPause(B)),
        Some(Command::PlayPause(B))
    ));
    assert!(matches!(
        run(&mut st, &s, Action::CuePress(A)),
        Some(Command::CuePress(A))
    ));
    assert!(matches!(
        run(&mut st, &s, Action::CueRelease(A)),
        Some(Command::CueRelease(A))
    ));
    assert!(matches!(
        run(&mut st, &s, Action::HotCue(A, 3)),
        Some(Command::HotCue(A, 3))
    ));
    assert!(matches!(
        run(&mut st, &s, Action::ClearHotCue(B, 1)),
        Some(Command::ClearHotCue(B, 1))
    ));
}

#[test]
fn crossfader_steps_accumulate_without_waiting_for_the_engine() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    run(&mut st, &s, Action::Crossfader(Dir::Up, false));
    let cmd = run(&mut st, &s, Action::Crossfader(Dir::Up, false));
    assert!(matches!(cmd, Some(Command::SetCrossfader(x)) if (x - 0.2).abs() < 1e-6));
    let cmd = run(&mut st, &s, Action::Crossfader(Dir::Down, true));
    assert!(matches!(cmd, Some(Command::SetCrossfader(x)) if x == -1.0));
    run(&mut st, &s, Action::Crossfader(Dir::Down, false));
    assert_eq!(st.crossfader, -1.0);
}

#[test]
fn channel_fader_steps_within_range() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    let cmd = run(&mut st, &s, Action::Fader(A, Dir::Up));
    assert!(matches!(cmd, Some(Command::SetChannelFader(A, v)) if v == 1.0));
    let cmd = run(&mut st, &s, Action::Fader(A, Dir::Down));
    assert!(matches!(cmd, Some(Command::SetChannelFader(A, v)) if (v - 0.95).abs() < 1e-6));
}

#[test]
fn headphone_cue_toggles() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    assert!(matches!(
        run(&mut st, &s, Action::HeadphoneCue(B)),
        Some(Command::SetHeadphoneCue(B, true))
    ));
    assert!(matches!(
        run(&mut st, &s, Action::HeadphoneCue(B)),
        Some(Command::SetHeadphoneCue(B, false))
    ));
}

#[test]
fn tempo_steps_and_is_limited_to_the_configured_range() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    run(&mut st, &s, Action::Tempo(A, Dir::Up, false));
    assert!((st.rates[0] - 1.005).abs() < 1e-9);
    let cmd = run(&mut st, &s, Action::Tempo(A, Dir::Down, true));
    assert!(matches!(cmd, Some(Command::SetRate(A, r)) if (r - 1.0045).abs() < 1e-9));
    for _ in 0..100 {
        run(&mut st, &s, Action::Tempo(A, Dir::Up, false));
    }
    assert!((st.rates[0] - 1.08).abs() < 1e-9);
    assert_eq!(st.rates[1], 1.0);
}

#[test]
fn seek_tenth_uses_the_loaded_track_length() {
    let mut st = ControlState::default();
    let s = snap_with_frames(1000, 0);
    assert!(
        matches!(run(&mut st, &s, Action::SeekTenth(A, 5)), Some(Command::Seek(A, f)) if f == 500.0)
    );
    assert!(run(&mut st, &s, Action::SeekTenth(B, 5)).is_none());
}

#[test]
fn actions_the_engine_cannot_do_yet_give_no_command() {
    let mut st = ControlState::default();
    assert!(run(&mut st, &Snapshot::default(), Action::FxToggle(A)).is_none());
}

#[test]
fn actions_flow_end_to_end_through_the_realtime_channel() {
    let (mut h, mut p) = channel(Engine::new(), 16);
    h.send(Command::Load(
        A,
        Arc::new(Track::from_interleaved(vec![0.0; 4000], 1000)),
    ))
    .unwrap();
    let mut buf = (vec![0.0; 20], vec![0.0; 20]);
    p.process(&mut buf.0, &mut buf.1);

    let mut st = ControlState::default();
    for action in [
        Action::SeekTenth(A, 5),
        Action::PlayPause(A),
        Action::Tempo(A, Dir::Up, false),
    ] {
        let snap = h.snapshot();
        if let Some(cmd) = apply(&mut st, &ctl(), &snap, action) {
            h.send(cmd).unwrap();
        }
    }
    p.process(&mut buf.0, &mut buf.1);
    let s = h.snapshot();
    assert!(s.decks[0].playing);
    assert!((s.decks[0].position - (1000.0 + 10.0 * 1.005)).abs() < 1e-6);
}

// --- Channel strip (M2) ---

use engine::dsp::EqBand;
use input::Band;

#[test]
fn trim_steps_one_db_within_plus_minus_twelve() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    assert!(matches!(run(&mut st, &s, Action::Trim(A, Dir::Up)), Some(Command::SetTrim(A, v)) if v == 1.0));
    for _ in 0..30 {
        run(&mut st, &s, Action::Trim(A, Dir::Up));
    }
    assert_eq!(st.strips[0].trim_db, 12.0);
    for _ in 0..30 {
        run(&mut st, &s, Action::Trim(A, Dir::Down));
    }
    assert_eq!(st.strips[0].trim_db, -12.0);
}

#[test]
fn eq_steps_two_db_from_kill_range_up_to_plus_six() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    let cmd = run(&mut st, &s, Action::Eq(B, Band::Low, Dir::Down));
    assert!(matches!(cmd, Some(Command::SetEq(B, EqBand::Low, v)) if v == -2.0));
    for _ in 0..10 {
        run(&mut st, &s, Action::Eq(B, Band::High, Dir::Up));
    }
    assert_eq!(st.strips[1].eq_db[EqBand::High as usize], 6.0);
    for _ in 0..40 {
        run(&mut st, &s, Action::Eq(B, Band::Mid, Dir::Down));
    }
    assert_eq!(st.strips[1].eq_db[EqBand::Mid as usize], -26.0);
}

#[test]
fn eq_kill_toggles() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    assert!(matches!(run(&mut st, &s, Action::EqKill(A, Band::Mid)), Some(Command::SetEqKill(A, EqBand::Mid, true))));
    assert!(matches!(run(&mut st, &s, Action::EqKill(A, Band::Mid)), Some(Command::SetEqKill(A, EqBand::Mid, false))));
}

#[test]
fn filter_steps_and_returns_exactly_to_centre() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    for _ in 0..3 {
        run(&mut st, &s, Action::Filter(A, Dir::Down));
    }
    assert!((st.strips[0].filter + 0.3).abs() < 1e-6);
    for _ in 0..3 {
        run(&mut st, &s, Action::Filter(A, Dir::Up));
    }
    assert_eq!(st.strips[0].filter, 0.0);
    for _ in 0..30 {
        run(&mut st, &s, Action::Filter(A, Dir::Up));
    }
    assert!(matches!(run(&mut st, &s, Action::Filter(A, Dir::Up)), Some(Command::SetFilter(A, v)) if v == 1.0));
}
