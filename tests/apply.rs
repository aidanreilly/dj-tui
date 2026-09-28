//! Turning input actions into engine commands. The UI owns the absolute values of mixer
//! controls and tempo, so repeated key presses between audio callbacks never get lost.

use dj_tui::apply::{apply, apply_many, ControlState, Controls};
use engine::{channel, Command, DeckId::*, Engine, Snapshot, Track};
use input::{Action, Dir};
use std::sync::Arc;

fn ctl() -> Controls {
    Controls::new(8, 48_000)
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

fn fader(cmd: Option<Command>, deck: engine::DeckId) -> f32 {
    match cmd {
        Some(Command::SetChannelFader(d, v)) if d == deck => v,
        other => panic!("expected a channel fader for {deck:?}, got {other:?}"),
    }
}

#[test]
fn the_fader_steps_in_decibels() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    // Unity is the top of the travel, so up from there changes nothing.
    assert_eq!(fader(run(&mut st, &s, Action::Fader(A, Dir::Up)), A), 1.0);
    // One step down is 2 dB, which is about 0.794 in amplitude.
    let v = fader(run(&mut st, &s, Action::Fader(A, Dir::Down)), A);
    assert!((v - 0.794).abs() < 1e-3, "got {v}");
    // Twenty steps reach the floor, and the next one is silence.
    for _ in 0..19 {
        run(&mut st, &s, Action::Fader(A, Dir::Down));
    }
    let floor = st.faders[0];
    assert!((floor - 0.01).abs() < 1e-3, "-40 dB, got {floor}");
    assert_eq!(fader(run(&mut st, &s, Action::Fader(A, Dir::Down)), A), 0.0);
}

#[test]
fn stepping_up_from_silence_returns_to_the_floor() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    st.faders[0] = 0.0;
    let v = fader(run(&mut st, &s, Action::Fader(A, Dir::Up)), A);
    assert!((v - 0.01).abs() < 1e-3, "-40 dB, got {v}");
}

#[test]
fn stepping_up_stops_at_unity() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    st.faders[0] = 0.0;
    for _ in 0..40 {
        run(&mut st, &s, Action::Fader(A, Dir::Up));
    }
    assert_eq!(st.faders[0], 1.0);
}

#[test]
fn shift_sends_the_fader_to_an_end() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    assert_eq!(
        fader(run(&mut st, &s, Action::FaderEnd(A, Dir::Down)), A),
        0.0
    );
    assert_eq!(
        fader(run(&mut st, &s, Action::FaderEnd(A, Dir::Up)), A),
        1.0
    );
}

#[test]
fn x_centres_the_crossfader_and_v_centres_the_filter() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    st.crossfader = -1.0;
    assert!(matches!(
        run(&mut st, &s, Action::CrossfaderCentre),
        Some(Command::SetCrossfader(x)) if x == 0.0
    ));
    st.strips[0].filter = 0.7;
    assert!(matches!(
        run(&mut st, &s, Action::FilterCentre(A)),
        Some(Command::SetFilter(A, f)) if f == 0.0
    ));
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

/// Both decks loaded, deck A at 120 BPM and deck B at 128, each with a grid on the first frame.
fn two_decks() -> (ControlState, Snapshot) {
    // 48 kHz: 24 000 frames a beat is 120 BPM, 22 500 is 128.
    let st = ControlState {
        beat_frames: [Some(24_000.0), Some(22_500.0)],
        ..Default::default()
    };
    (st, snap_with_frames(480_000, 480_000))
}

#[test]
fn nudging_shifts_the_playhead_a_little_either_way() {
    let (mut st, mut s) = two_decks();
    s.decks[0].position = 100_000.0;
    let forward = seek_frame(run(&mut st, &s, Action::Nudge(A, Dir::Up)), A);
    let back = seek_frame(run(&mut st, &s, Action::Nudge(A, Dir::Down)), A);
    assert!(forward > 100_000.0 && back < 100_000.0, "{forward} {back}");
    assert_eq!(
        forward - 100_000.0,
        100_000.0 - back,
        "the same step either way"
    );
    // Small enough to beatmatch with, not a jump.
    assert!(forward - 100_000.0 < 24_000.0 / 8.0, "{forward}");
}

#[test]
fn a_nudge_stays_inside_the_track() {
    let (mut st, mut s) = two_decks();
    s.decks[0].position = 5.0;
    assert_eq!(
        seek_frame(run(&mut st, &s, Action::Nudge(A, Dir::Down)), A),
        0.0
    );
}

#[test]
fn sync_matches_the_other_decks_tempo() {
    let (mut st, mut s) = two_decks();
    s.decks[0].position = 0.0;
    s.decks[1].position = 0.0;
    // Deck A is the one being synced, so it takes deck B's tempo.
    let cmd = run(&mut st, &s, Action::Sync(A));
    let rate = match cmd {
        Some(Command::SetRate(A, r)) => r,
        other => panic!("expected a rate for deck A, got {other:?}"),
    };
    assert!(
        (rate - 24_000.0 / 22_500.0).abs() < 1e-9,
        "120 BPM pulled up to 128: {rate}"
    );
    assert!((st.rates[0] - rate).abs() < 1e-9, "the UI keeps the value");
}

#[test]
fn sync_lines_the_beats_up_after_the_tempo_matches() {
    let (mut st, mut s) = two_decks();
    // Deck B is a quarter of a beat past its downbeat, deck A is on one. B is the reference,
    // so it is running: a stopped deck has no phase to line up with.
    s.decks[1].position = 22_500.0 / 4.0;
    s.decks[1].playing = true;
    s.decks[0].position = 48_000.0;
    run(&mut st, &s, Action::Sync(A));
    // The second press, with tempo already matched, moves the playhead into phase.
    let target = seek_frame(run(&mut st, &s, Action::Sync(A)), A);
    let beat = 24_000.0;
    let phase = ((target - 0.0) / beat).rem_euclid(1.0);
    assert!(
        (phase - 0.25).abs() < 1e-6,
        "deck A ends up a quarter beat in, like deck B: {phase}"
    );
    assert!(
        (target - 48_000.0).abs() <= beat / 2.0,
        "and gets there by the shortest move: {target}"
    );
}

#[test]
fn sync_needs_a_grid_on_both_decks() {
    let mut st = ControlState::default();
    let s = snap_with_frames(480_000, 480_000);
    assert!(run(&mut st, &s, Action::Sync(A)).is_none());
    st.beat_frames[0] = Some(24_000.0);
    assert!(
        run(&mut st, &s, Action::Sync(A)).is_none(),
        "one grid is not enough"
    );
}

#[test]
fn sync_will_not_pull_the_tempo_past_the_fader_range() {
    let (mut st, mut s) = two_decks();
    // Deck B at 180 BPM against deck A's 120 is more than the ±8 % fader allows.
    st.beat_frames[1] = Some(16_000.0);
    s.decks[0].position = 0.0;
    s.decks[1].position = 0.0;
    let rate = match run(&mut st, &s, Action::Sync(A)) {
        Some(Command::SetRate(A, r)) => r,
        other => panic!("got {other:?}"),
    };
    assert!((rate - 1.08).abs() < 1e-9, "clamped to the range: {rate}");
}

#[test]
fn the_headphone_mix_blends_between_the_cue_bus_and_the_master() {
    let mut st = ControlState::default();
    let s = Snapshot::default();
    assert_eq!(st.cue_mix, 0.5, "half and half to start");
    let value = |cmd: Option<Command>| match cmd {
        Some(Command::SetCueMix(v)) => v,
        other => panic!("expected a cue mix, got {other:?}"),
    };
    assert!((value(run(&mut st, &s, Action::CueMix(Dir::Up))) - 0.6).abs() < 1e-6);
    assert_eq!(st.cue_mix, 0.6);
    for _ in 0..10 {
        run(&mut st, &s, Action::CueMix(Dir::Up));
    }
    assert_eq!(st.cue_mix, 1.0, "all master");
    for _ in 0..20 {
        run(&mut st, &s, Action::CueMix(Dir::Down));
    }
    assert_eq!(st.cue_mix, 0.0, "all cue");
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

mod absolute {
    use super::*;
    use dj_tui::apply::set_control;
    use midi::Control;

    fn set(st: &mut ControlState, control: Control, value: f32) -> Option<Command> {
        set_control(st, &ctl(), control, value)
    }

    #[test]
    fn a_fader_goes_where_it_is_put() {
        let mut st = ControlState::default();
        assert!(matches!(
            set(&mut st, Control::Fader(A), 0.25),
            Some(Command::SetChannelFader(A, v)) if (v - 0.25).abs() < 1e-6
        ));
        assert_eq!(st.faders[0], 0.25, "the UI keeps what it was given");
    }

    #[test]
    fn the_crossfader_spans_both_ends() {
        let mut st = ControlState::default();
        set(&mut st, Control::Crossfader, 0.0);
        assert_eq!(st.crossfader, -1.0);
        set(&mut st, Control::Crossfader, 1.0);
        assert_eq!(st.crossfader, 1.0);
        set(&mut st, Control::Crossfader, 0.5);
        assert_eq!(st.crossfader, 0.0, "the middle is the middle");
    }

    #[test]
    fn the_tempo_knob_covers_the_configured_range() {
        let mut st = ControlState::default();
        set(&mut st, Control::Tempo(B), 1.0);
        assert!((st.rates[1] - 1.08).abs() < 1e-6, "{}", st.rates[1]);
        set(&mut st, Control::Tempo(B), 0.0);
        assert!((st.rates[1] - 0.92).abs() < 1e-6, "{}", st.rates[1]);
        set(&mut st, Control::Tempo(B), 0.5);
        assert!((st.rates[1] - 1.0).abs() < 1e-6, "centred is normal speed");
    }

    #[test]
    fn eq_trim_and_filter_land_on_their_own_ranges() {
        let mut st = ControlState::default();
        set(&mut st, Control::Eq(A, Band::High), 1.0);
        assert_eq!(st.strips[0].eq_db[2], 6.0, "the EQ tops out at +6 dB");
        set(&mut st, Control::Eq(A, Band::High), 0.0);
        assert!(st.strips[0].eq_db[2] <= -26.0, "and bottoms out at a kill");
        set(&mut st, Control::Trim(A), 1.0);
        assert_eq!(st.strips[0].trim_db, 12.0);
        set(&mut st, Control::Filter(A), 0.5);
        assert_eq!(st.strips[0].filter, 0.0, "centred is no filter");
        set(&mut st, Control::Filter(A), 0.0);
        assert_eq!(st.strips[0].filter, -1.0);
    }

    #[test]
    fn values_outside_the_range_are_pulled_back_in() {
        let mut st = ControlState::default();
        set(&mut st, Control::Fader(A), 4.0);
        assert_eq!(st.faders[0], 1.0);
        set(&mut st, Control::Fader(A), -4.0);
        assert_eq!(st.faders[0], 0.0);
    }

    #[test]
    fn a_knob_can_set_the_headphone_mix_too() {
        let mut st = ControlState::default();
        set(&mut st, Control::CueMix, 0.25);
        assert_eq!(st.cue_mix, 0.25);
        assert_eq!(
            dj_tui::apply::control_value(&st, &ctl(), Control::CueMix),
            0.25,
            "and reads back for takeover"
        );
    }

    #[test]
    fn what_the_ui_holds_reads_back_for_soft_takeover() {
        use dj_tui::apply::control_value;
        let mut st = ControlState::default();
        set(&mut st, Control::Fader(A), 0.3);
        set(&mut st, Control::Crossfader, 0.25);
        set(&mut st, Control::Tempo(A), 0.75);
        assert!((control_value(&st, &ctl(), Control::Fader(A)) - 0.3).abs() < 1e-6);
        assert!((control_value(&st, &ctl(), Control::Crossfader) - 0.25).abs() < 1e-6);
        assert!((control_value(&st, &ctl(), Control::Tempo(A)) - 0.75).abs() < 1e-6);
    }
}

/// A deck that has run out is still a grid but no longer a clock: its position sits pinned at
/// the end. Lining the beats up against it would jerk the playhead to match a frozen phase.
mod sync_against_a_stopped_deck {
    use super::*;

    /// Both decks analysed at the same tempo, A stopped at the end of its track, B playing.
    fn state_and_snapshot() -> (ControlState, Snapshot) {
        let st = ControlState {
            beat_frames: [Some(24_000.0); 2],
            first_beat_frames: [0.0; 2],
            ..Default::default()
        };
        let mut s = Snapshot::default();
        for d in s.decks.iter_mut() {
            d.track_frames = 480_000;
        }
        s.decks[0].position = 480_000.0;
        s.decks[0].playing = false;
        s.decks[1].position = 6_000.0;
        s.decks[1].playing = true;
        (st, s)
    }

    #[test]
    fn matching_tempo_against_a_stopped_deck_still_works() {
        // Its BPM and its tempo fader are both known whether it is running or not, and
        // matching the beat length it was playing at means running at the same rate.
        let (mut st, s) = state_and_snapshot();
        st.rates[0] = 1.04;
        let cmd = run(&mut st, &s, Action::Sync(B));
        assert!(
            matches!(cmd, Some(Command::SetRate(B, r)) if (r - 1.04).abs() < 1e-6),
            "got {cmd:?}"
        );
    }

    #[test]
    fn lining_the_beats_up_against_a_stopped_deck_does_nothing() {
        let (mut st, s) = state_and_snapshot();
        // Tempo already matches, so this press would be the phase alignment.
        let cmd = run(&mut st, &s, Action::Sync(B));
        assert!(
            cmd.is_none(),
            "a stopped deck has no phase to line up with, got {cmd:?}"
        );
    }

    #[test]
    fn lining_the_beats_up_against_a_running_deck_still_seeks() {
        let (mut st, mut s) = state_and_snapshot();
        s.decks[0].playing = true;
        s.decks[0].position = 1_000.0;
        let cmd = run(&mut st, &s, Action::Sync(B));
        assert!(
            matches!(cmd, Some(Command::Seek(B, _))),
            "two running decks still line up, got {cmd:?}"
        );
    }
}

/// `+`/`-` ride both decks together, the way you would ride two pitch faders on records.
mod global_tempo {
    use super::*;

    fn matched() -> ControlState {
        ControlState {
            beat_frames: [Some(24_000.0), Some(24_000.0)],
            ..Default::default()
        }
    }

    fn rates(cmds: &[Command]) -> Vec<(engine::DeckId, f64)> {
        cmds.iter()
            .map(|c| match c {
                Command::SetRate(d, r) => (*d, *r),
                other => panic!("expected a rate, got {other:?}"),
            })
            .collect()
    }

    #[test]
    fn one_press_moves_both_decks_by_the_same_proportion() {
        let mut st = matched();
        let s = Snapshot::default();
        let cmds = apply_many(&mut st, &ctl(), &s, Action::GlobalTempo(Dir::Up, false));
        let got = rates(&cmds);
        assert_eq!(got.len(), 2, "both decks move: {got:?}");
        assert!((got[0].1 - 1.005).abs() < 1e-9, "{got:?}");
        assert!((got[1].1 - 1.005).abs() < 1e-9, "{got:?}");
    }

    #[test]
    fn a_matched_pair_stays_matched_however_far_it_is_ridden() {
        let mut st = matched();
        // Deck B was beatmatched to a slightly different cut, so the rates differ.
        st.rates = [1.0, 1.02];
        let s = Snapshot::default();
        let before = st.rates[1] / st.rates[0];
        for _ in 0..8 {
            apply_many(&mut st, &ctl(), &s, Action::GlobalTempo(Dir::Up, false));
        }
        let after = st.rates[1] / st.rates[0];
        assert!(
            (after - before).abs() < 1e-12,
            "the ratio between the decks drifted: {before} to {after}"
        );
    }

    #[test]
    fn riding_up_and_back_returns_to_where_it_started() {
        let mut st = matched();
        st.rates = [1.0, 1.02];
        let s = Snapshot::default();
        for _ in 0..6 {
            apply_many(&mut st, &ctl(), &s, Action::GlobalTempo(Dir::Up, false));
        }
        for _ in 0..6 {
            apply_many(&mut st, &ctl(), &s, Action::GlobalTempo(Dir::Down, false));
        }
        assert_eq!(st.rates[0], 1.0, "normal speed reads as exactly normal");
        assert!((st.rates[1] - 1.02).abs() < 1e-12, "got {}", st.rates[1]);
    }

    #[test]
    fn the_fine_step_is_smaller() {
        let mut coarse = matched();
        let mut fine = matched();
        let s = Snapshot::default();
        apply_many(&mut coarse, &ctl(), &s, Action::GlobalTempo(Dir::Up, false));
        apply_many(&mut fine, &ctl(), &s, Action::GlobalTempo(Dir::Up, true));
        assert!(fine.rates[0] > 1.0);
        assert!(fine.rates[0] < coarse.rates[0]);
    }

    /// The whole move is refused rather than clamping one deck and letting the pair come
    /// apart, which would be worse than not moving at all.
    #[test]
    fn the_move_is_refused_when_either_deck_would_hit_its_range() {
        let mut st = matched();
        // Deck B is already at the top of a +/-8% fader; deck A has room.
        st.rates = [1.0, 1.08];
        let s = Snapshot::default();
        let before = st.rates;
        let cmds = apply_many(&mut st, &ctl(), &s, Action::GlobalTempo(Dir::Up, false));
        assert!(cmds.is_empty(), "nothing is sent: {cmds:?}");
        assert_eq!(st.rates, before, "and nothing moves");

        // Downward still works, since that is away from the limit.
        let cmds = apply_many(&mut st, &ctl(), &s, Action::GlobalTempo(Dir::Down, false));
        assert_eq!(cmds.len(), 2, "{cmds:?}");
    }

    #[test]
    fn a_deck_with_no_track_does_not_hold_the_other_one_back() {
        // Riding the tempo should work with one deck loaded, which is most of a set.
        let mut st = ControlState {
            beat_frames: [Some(24_000.0), None],
            ..Default::default()
        };
        let mut s = Snapshot::default();
        s.decks[0].track_frames = 480_000;
        let cmds = apply_many(&mut st, &ctl(), &s, Action::GlobalTempo(Dir::Up, false));
        assert_eq!(rates(&cmds), vec![(A, 1.005)], "only the loaded deck moves");
    }
}
