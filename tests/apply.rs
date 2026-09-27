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

/// The span of a `SetLoop` for `deck`, panicking on anything else. `Command` holds an `Arc`
/// and so has no `PartialEq`, which is why these tests unwrap the command they expect.
fn loop_span(cmd: Option<Command>, deck: engine::DeckId) -> Option<(f64, f64)> {
    match cmd {
        Some(Command::SetLoop(d, span)) if d == deck => span,
        other => panic!("expected a loop for deck {deck:?}, got {other:?}"),
    }
}

fn seek_frame(cmd: Option<Command>, deck: engine::DeckId) -> f64 {
    match cmd {
        Some(Command::Seek(d, frame)) if d == deck => frame,
        other => panic!("expected a seek on deck {deck:?}, got {other:?}"),
    }
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

/// A deck with a track and a known beat length, so loops land on predictable frames.
fn snap_with_beats(frames: usize) -> (ControlState, Snapshot) {
    let mut st = ControlState::default();
    st.beat_frames[0] = Some(24_000.0);
    (st, snap_with_frames(frames, 0))
}

#[test]
fn loop_toggles_a_four_beat_loop_from_the_playhead_and_off_again() {
    let (mut st, mut s) = snap_with_beats(480_000);
    s.decks[0].position = 96_000.0;
    let cmd = run(&mut st, &s, Action::LoopToggle(A));
    assert_eq!(
        loop_span(cmd, A),
        Some((96_000.0, 96_000.0 + 4.0 * 24_000.0))
    );

    // With the loop running, the same key clears it.
    s.decks[0].loop_span = Some((96_000.0, 192_000.0));
    assert_eq!(loop_span(run(&mut st, &s, Action::LoopToggle(A)), A), None);
}

#[test]
fn a_loop_needs_a_beat_grid() {
    let mut st = ControlState::default();
    let s = snap_with_frames(480_000, 0);
    assert!(run(&mut st, &s, Action::LoopToggle(A)).is_none());
    assert!(run(&mut st, &s, Action::BeatJump(A, Dir::Up)).is_none());
}

#[test]
fn halving_and_doubling_keep_the_loop_in_point() {
    let (mut st, mut s) = snap_with_beats(480_000);
    s.decks[0].position = 24_000.0;
    run(&mut st, &s, Action::LoopToggle(A));
    s.decks[0].loop_span = Some((24_000.0, 24_000.0 + 96_000.0));

    assert_eq!(
        loop_span(run(&mut st, &s, Action::LoopHalve(A)), A),
        Some((24_000.0, 24_000.0 + 48_000.0)),
        "two beats from the same in point"
    );
    s.decks[0].loop_span = Some((24_000.0, 24_000.0 + 48_000.0));
    assert_eq!(
        loop_span(run(&mut st, &s, Action::LoopDouble(A)), A),
        Some((24_000.0, 24_000.0 + 96_000.0))
    );
}

#[test]
fn halving_and_doubling_without_a_loop_only_set_the_length_for_the_next_one() {
    let (mut st, mut s) = snap_with_beats(480_000);
    assert!(run(&mut st, &s, Action::LoopDouble(A)).is_none());
    assert_eq!(st.loop_beats[0], 8.0);
    s.decks[0].position = 0.0;
    assert_eq!(
        loop_span(run(&mut st, &s, Action::LoopToggle(A)), A),
        Some((0.0, 8.0 * 24_000.0))
    );
}

#[test]
fn loop_length_stops_at_an_eighth_of_a_beat_and_at_thirty_two() {
    let (mut st, s) = snap_with_beats(480_000);
    for _ in 0..8 {
        run(&mut st, &s, Action::LoopHalve(A));
    }
    assert_eq!(st.loop_beats[0], 0.125);
    for _ in 0..10 {
        run(&mut st, &s, Action::LoopDouble(A));
    }
    assert_eq!(st.loop_beats[0], 32.0);
}

#[test]
fn beat_jump_moves_by_the_loop_length_and_stays_in_the_track() {
    let (mut st, mut s) = snap_with_beats(480_000);
    s.decks[0].position = 240_000.0;
    assert_eq!(
        seek_frame(run(&mut st, &s, Action::BeatJump(A, Dir::Up)), A),
        240_000.0 + 96_000.0
    );
    assert_eq!(
        seek_frame(run(&mut st, &s, Action::BeatJump(A, Dir::Down)), A),
        240_000.0 - 96_000.0
    );
    s.decks[0].position = 10_000.0;
    assert_eq!(
        seek_frame(run(&mut st, &s, Action::BeatJump(A, Dir::Down)), A),
        0.0,
        "a jump back past the start lands on the start"
    );
}

#[test]
fn quantize_snaps_loops_and_jumps_to_the_nearest_beat() {
    let (mut st, mut s) = snap_with_beats(480_000);
    // The grid's first beat sits 1000 frames in, so beats fall on 1000, 25_000, 49_000 …
    st.first_beat_frames[0] = 1_000.0;
    st.quantize[0] = true;
    s.decks[0].position = 26_000.0;
    assert_eq!(
        loop_span(run(&mut st, &s, Action::LoopToggle(A)), A),
        Some((25_000.0, 25_000.0 + 96_000.0)),
        "the loop in point snaps back to the beat at 25_000"
    );
    assert_eq!(
        seek_frame(run(&mut st, &s, Action::BeatJump(A, Dir::Up)), A),
        25_000.0 + 96_000.0
    );
}

#[test]
fn a_manual_loop_runs_from_the_in_point_to_where_the_out_key_lands() {
    let (mut st, mut s) = snap_with_beats(480_000);
    s.decks[0].position = 50_000.0;
    assert!(
        run(&mut st, &s, Action::LoopIn(A)).is_none(),
        "the in point alone changes nothing the engine can play"
    );
    assert_eq!(st.loop_in[0], Some(50_000.0));

    s.decks[0].position = 146_000.0;
    assert_eq!(
        loop_span(run(&mut st, &s, Action::LoopOut(A)), A),
        Some((50_000.0, 146_000.0))
    );
    assert_eq!(st.loop_in[0], None, "the in point is used up");
}

#[test]
fn a_manual_loop_sets_the_length_the_halve_and_double_keys_work_from() {
    let (mut st, mut s) = snap_with_beats(480_000);
    s.decks[0].position = 0.0;
    run(&mut st, &s, Action::LoopIn(A));
    s.decks[0].position = 48_000.0;
    run(&mut st, &s, Action::LoopOut(A));
    assert_eq!(st.loop_beats[0], 2.0, "two beats long");
}

#[test]
fn the_out_key_needs_an_in_point_ahead_of_it() {
    let (mut st, mut s) = snap_with_beats(480_000);
    s.decks[0].position = 10_000.0;
    assert!(
        run(&mut st, &s, Action::LoopOut(A)).is_none(),
        "no in point, no loop"
    );

    run(&mut st, &s, Action::LoopIn(A));
    s.decks[0].position = 5_000.0;
    assert!(
        run(&mut st, &s, Action::LoopOut(A)).is_none(),
        "an out point behind the in point is not a loop"
    );
    assert_eq!(st.loop_in[0], Some(10_000.0), "the in point is still there");
}

#[test]
fn the_in_key_during_a_loop_drops_it_and_starts_a_new_one() {
    let (mut st, mut s) = snap_with_beats(480_000);
    s.decks[0].loop_span = Some((0.0, 96_000.0));
    s.decks[0].position = 200_000.0;
    assert_eq!(
        loop_span(run(&mut st, &s, Action::LoopIn(A)), A),
        None,
        "the running loop is cleared"
    );
    assert_eq!(st.loop_in[0], Some(200_000.0));
}

#[test]
fn manual_loop_points_snap_to_the_beat_when_quantize_is_on() {
    let (mut st, mut s) = snap_with_beats(480_000);
    st.first_beat_frames[0] = 1_000.0;
    st.quantize[0] = true;
    s.decks[0].position = 26_000.0;
    run(&mut st, &s, Action::LoopIn(A));
    assert_eq!(st.loop_in[0], Some(25_000.0));
    s.decks[0].position = 120_000.0;
    assert_eq!(
        loop_span(run(&mut st, &s, Action::LoopOut(A)), A),
        Some((25_000.0, 121_000.0))
    );
}

#[test]
fn a_manual_loop_works_without_a_beat_grid() {
    let mut st = ControlState::default();
    let mut s = snap_with_frames(480_000, 0);
    s.decks[0].position = 1_234.0;
    run(&mut st, &s, Action::LoopIn(A));
    s.decks[0].position = 9_999.0;
    assert_eq!(
        loop_span(run(&mut st, &s, Action::LoopOut(A)), A),
        Some((1_234.0, 9_999.0)),
        "marking both ends by hand needs no analysis"
    );
}

#[test]
fn quantize_toggles_without_sending_a_command() {
    let (mut st, s) = snap_with_beats(480_000);
    assert!(run(&mut st, &s, Action::Quantize(A)).is_none());
    assert!(st.quantize[0]);
    assert!(run(&mut st, &s, Action::Quantize(A)).is_none());
    assert!(!st.quantize[0]);
}

#[test]
fn actions_the_engine_cannot_do_yet_give_no_command() {
    let mut st = ControlState::default();
    assert!(run(&mut st, &Snapshot::default(), Action::Sync(A)).is_none());
}

#[test]
fn the_effect_keys_switch_the_slot_cycle_it_and_set_the_wet() {
    use engine::fx::FxKind;
    let mut st = ControlState::default();
    let s = Snapshot::default();
    let fx = |cmd: Option<Command>| -> String { format!("{:?}", cmd.unwrap()) };

    assert!(fx(run(&mut st, &s, Action::FxToggle(A))).contains("SetFxOn(A, true)"));
    assert!(st.fx[0].on);
    assert!(fx(run(&mut st, &s, Action::FxToggle(A))).contains("SetFxOn(A, false)"));
    assert!(!st.fx[0].on);

    let first = st.fx[0].kind;
    assert!(fx(run(&mut st, &s, Action::FxNext(A))).contains("SetFxKind"));
    assert_eq!(st.fx[0].kind, first.next());
    assert_eq!(st.fx[1].kind, first, "each deck keeps its own slot");

    assert!(fx(run(&mut st, &s, Action::FxWet(A, Dir::Up))).contains("SetFxWet"));
    assert!((st.fx[0].wet - 0.1).abs() < 1e-6);
    for _ in 0..20 {
        run(&mut st, &s, Action::FxWet(A, Dir::Up));
    }
    assert_eq!(st.fx[0].wet, 1.0, "wet stops at full");
    for _ in 0..20 {
        run(&mut st, &s, Action::FxWet(A, Dir::Down));
    }
    assert_eq!(st.fx[0].wet, 0.0, "and at dry");
    assert_eq!(FxKind::default(), first);
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
    assert!(
        matches!(run(&mut st, &s, Action::Trim(A, Dir::Up)), Some(Command::SetTrim(A, v)) if v == 1.0)
    );
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
    assert!(matches!(
        run(&mut st, &s, Action::EqKill(A, Band::Mid)),
        Some(Command::SetEqKill(A, EqBand::Mid, true))
    ));
    assert!(matches!(
        run(&mut st, &s, Action::EqKill(A, Band::Mid)),
        Some(Command::SetEqKill(A, EqBand::Mid, false))
    ));
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
    assert!(
        matches!(run(&mut st, &s, Action::Filter(A, Dir::Up)), Some(Command::SetFilter(A, v)) if v == 1.0)
    );
}
