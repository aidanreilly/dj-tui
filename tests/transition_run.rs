//! Arming and running a plan: taps, the beat clock, and what comes out each frame.

use analysis::phrase::{Phrases, Section, SectionKind};
use analysis::tempo::BeatGrid;
use dj_tui::automation::Curve;
use dj_tui::transition::run::{Effect, Press, SlotState, Transitions, DOUBLE_TAP_SECS};
use dj_tui::transition::{default_slots, Context, DeckInfo};
use engine::DeckId::{A, B};
use input::{Band, Control};

const SR: u32 = 48_000;
const BEAT: f64 = 24_000.0;
const BAR: f64 = 4.0 * BEAT;
/// Slots as they come: 1 bass swap, 4 jump cut, 8 brake, 9 auto.
const BASS_SWAP: usize = 0;
const JUMP_CUT: usize = 3;
const LOOP_OUT: usize = 5;
const BRAKE: usize = 7;

fn body() -> Phrases {
    Phrases {
        downbeat_beat: 0,
        bar_levels: vec![[1.0; 3]; 128],
        sections: vec![Section {
            kind: SectionKind::Body,
            start_bar: 0,
            bars: 128,
        }],
    }
}

fn deck(p: &Phrases) -> DeckInfo<'_> {
    DeckInfo {
        loaded: true,
        playing: false,
        position: 0.0,
        track_frames: 128.0 * BAR,
        rate: 1.0,
        key_lock: false,
        grid: Some(BeatGrid {
            bpm: 120.0,
            first_beat_secs: 0.0,
        }),
        phrases: p,
        cue_point: 0.0,
        hot_cues: [None; 8],
        key: None,
        fader: 1.0,
    }
}

fn ctx(p: &Phrases, a_bar: f64) -> Context<'_> {
    let mut a = deck(p);
    a.playing = true;
    a.position = a_bar * BAR;
    let mut b = deck(p);
    b.hot_cues[0] = Some(4.0 * BAR);
    Context {
        decks: [a, b],
        crossfader: 0.0,
        sample_rate: SR,
        tempo_range: 0.08,
    }
}

fn armed(slot: usize, p: &Phrases, a_bar: f64) -> Transitions {
    let mut t = Transitions::new(default_slots());
    assert!(matches!(
        t.press(slot, 0.0, &ctx(p, a_bar)),
        Press::Armed { .. }
    ));
    t
}

/// Move deck A to `bar` and run one frame of `dt` seconds.
fn frame(t: &mut Transitions, p: &Phrases, bar: f64, dt: f64) -> Vec<Effect> {
    let mut out = Vec::new();
    t.tick(dt, &ctx(p, bar), &mut out);
    out
}

#[test]
fn a_tap_arms_and_a_quick_second_tap_disarms_and_switches_the_slot_off() {
    let p = body();
    let c = ctx(&p, 10.5);
    let mut t = Transitions::new(default_slots());
    assert!(matches!(t.press(BASS_SWAP, 1.0, &c), Press::Armed { .. }));
    assert_eq!(t.armed_slot(), Some(BASS_SWAP));
    assert_eq!(
        t.press(BASS_SWAP, 1.0 + DOUBLE_TAP_SECS / 2.0, &c),
        Press::Disabled
    );
    assert_eq!(t.armed_slot(), None);
    assert!(!t.enabled(BASS_SWAP));
    assert_eq!(t.lines(&c)[BASS_SWAP].state, SlotState::Disabled);
}

#[test]
fn a_switched_off_slot_says_how_to_get_it_back_and_a_double_tap_does() {
    let p = body();
    let c = ctx(&p, 10.5);
    let mut t = Transitions::new(default_slots());
    t.press(BASS_SWAP, 1.0, &c);
    t.press(BASS_SWAP, 1.1, &c);
    match t.press(BASS_SWAP, 5.0, &c) {
        Press::Refused(why) => assert!(why.contains("double tap"), "{why}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(t.press(BASS_SWAP, 5.1, &c), Press::Enabled);
    assert!(t.enabled(BASS_SWAP));
    assert_eq!(t.armed_slot(), None, "switching back on does not arm");
}

#[test]
fn a_slow_second_tap_disarms_without_switching_off() {
    let p = body();
    let c = ctx(&p, 10.5);
    let mut t = Transitions::new(default_slots());
    t.press(BASS_SWAP, 1.0, &c);
    assert_eq!(t.press(BASS_SWAP, 3.0, &c), Press::Disarmed);
    assert!(t.enabled(BASS_SWAP));
}

#[test]
fn another_slot_replaces_the_armed_one() {
    let p = body();
    let c = ctx(&p, 10.5);
    let mut t = Transitions::new(default_slots());
    t.press(BASS_SWAP, 1.0, &c);
    assert!(matches!(t.press(JUMP_CUT, 1.1, &c), Press::Armed { .. }));
    assert_eq!(t.armed_slot(), Some(JUMP_CUT));
    assert!(t.enabled(BASS_SWAP));
}

#[test]
fn a_slot_with_nothing_to_do_refuses_and_says_why() {
    let p = body();
    let mut c = ctx(&p, 10.5);
    c.decks[0].playing = false;
    let mut t = Transitions::new(default_slots());
    match t.press(BASS_SWAP, 1.0, &c) {
        Press::Refused(why) => assert!(why.contains("Nothing is playing"), "{why}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(t.armed_slot(), None);
    assert_eq!(t.lines(&c)[BASS_SWAP].state, SlotState::Unavailable);
}

#[test]
fn nothing_moves_until_a_beat_before_the_start() {
    let p = body();
    let mut t = armed(BASS_SWAP, &p, 10.5);
    // The start is bar 16. A beat ahead of it is bar 15.75.
    assert!(frame(&mut t, &p, 15.5, 0.03).is_empty());
    let prep = frame(&mut t, &p, 15.76, 0.03);
    assert!(prep.contains(&Effect::Set {
        control: Control::Eq(B, Band::Low),
        to: 0.0
    }));
    assert!(prep.contains(&Effect::Set {
        control: Control::Fader(B),
        to: 0.0
    }));
}

#[test]
fn the_incoming_deck_starts_at_its_way_in_plus_however_late_the_frame_was() {
    let p = body();
    let mut t = armed(BASS_SWAP, &p, 10.5);
    frame(&mut t, &p, 15.9, 0.03);
    // A frame lands a tenth of a beat past the downbeat.
    let out = frame(&mut t, &p, 16.0 + 0.1 / 4.0, 0.03);
    let start = out
        .iter()
        .find_map(|e| match e {
            Effect::Start { deck, seek, .. } => Some((*deck, *seek)),
            _ => None,
        })
        .expect("a start");
    assert_eq!(start.0, B);
    let seek = start.1.unwrap();
    assert!((seek - (4.0 * BAR + 0.1 * BEAT)).abs() < 1.0, "{seek}");
    assert!(t.running());
}

#[test]
fn fades_are_timed_in_seconds_of_the_outgoing_beat() {
    let p = body();
    let mut t = armed(BASS_SWAP, &p, 10.5);
    let out = frame(&mut t, &p, 16.0, 0.03);
    // Sixteen bars at 120 BPM: the incoming fader comes up over the first 32 beats.
    assert!(out.contains(&Effect::Fade {
        control: Control::Fader(B),
        to: 1.0,
        secs: 16.0,
        curve: Curve::EqualPower,
    }));
}

#[test]
fn once_started_the_clock_runs_on_beats_not_the_outgoing_playhead() {
    let p = body();
    let mut t = armed(BASS_SWAP, &p, 10.5);
    frame(&mut t, &p, 16.0, 0.03);
    // The playhead jumping (a loop wrapping, a hot cue) must not stall or rewind the mix.
    let mut swapped = false;
    for _ in 0..(16.0 / 0.05) as usize + 2 {
        let out = frame(&mut t, &p, 3.0, 0.05);
        swapped |= out.contains(&Effect::Set {
            control: Control::Eq(A, Band::Low),
            to: 0.0,
        });
    }
    assert!(swapped, "the lows swap 32 beats, 16 s, in");
}

#[test]
fn a_finished_mix_stops_the_old_deck_and_lets_go() {
    let p = body();
    let mut t = armed(JUMP_CUT, &p, 10.5);
    let out = frame(&mut t, &p, 12.0, 0.03);
    assert!(out.contains(&Effect::Stop { deck: A }));
    assert!(!t.running());
    assert_eq!(t.armed_slot(), None);
}

#[test]
fn a_tap_while_a_mix_runs_is_refused() {
    let p = body();
    let mut t = armed(BASS_SWAP, &p, 10.5);
    frame(&mut t, &p, 16.0, 0.03);
    let c = ctx(&p, 16.1);
    assert!(matches!(t.press(JUMP_CUT, 10.0, &c), Press::Refused(_)));
}

#[test]
fn a_control_taken_by_hand_is_left_alone_from_then_on() {
    let p = body();
    let mut t = armed(BASS_SWAP, &p, 10.5);
    frame(&mut t, &p, 16.0, 0.03);
    t.release(Control::Eq(A, Band::Low));
    let mut touched = false;
    for _ in 0..400 {
        let out = frame(&mut t, &p, 16.0, 0.05);
        touched |= out.iter().any(|e| {
            matches!(
                e,
                Effect::Set {
                    control: Control::Eq(A, Band::Low),
                    ..
                }
            ) | matches!(
                e,
                Effect::Fade {
                    control: Control::Eq(A, Band::Low),
                    ..
                }
            )
        });
    }
    assert!(!touched);
}

#[test]
fn cancel_drops_the_plan_where_it_stands() {
    let p = body();
    let mut t = armed(BASS_SWAP, &p, 10.5);
    assert!(t.cancel());
    assert!(frame(&mut t, &p, 16.0, 0.03).is_empty());
    assert!(!t.cancel(), "nothing left to cancel");
}

#[test]
fn a_seek_past_the_start_before_it_comes_drops_the_plan() {
    let p = body();
    let mut t = armed(BASS_SWAP, &p, 10.5);
    let mut out = Vec::new();
    let msg = t.tick(0.03, &ctx(&p, 40.0), &mut out);
    assert!(out.is_empty());
    assert!(msg.is_some_and(|m| m.contains("jumped")));
    assert_eq!(t.armed_slot(), None);
}

#[test]
fn a_brake_winds_the_rate_down_then_puts_it_back() {
    let p = body();
    let mut t = armed(BRAKE, &p, 10.5);
    // Start is bar 12. The wind-down begins two beats ahead.
    frame(&mut t, &p, 11.5, 0.03);
    let mut rates = Vec::new();
    for _ in 0..40 {
        for e in frame(&mut t, &p, 11.5, 0.05) {
            if let Effect::Rate { deck: A, rate } = e {
                rates.push(rate);
            }
        }
    }
    assert!(rates.windows(2).take(5).all(|w| w[1] < w[0]), "{rates:?}");
    assert_eq!(rates.last(), Some(&1.0), "back to where it was");
}

#[test]
fn a_loop_out_loops_from_the_start_point() {
    let p = body();
    let mut t = armed(LOOP_OUT, &p, 10.5);
    let out = frame(&mut t, &p, 16.0, 0.03);
    assert!(out.contains(&Effect::Loop {
        deck: A,
        span: Some((16.0 * BAR, 20.0 * BAR))
    }));
}

#[test]
fn the_lines_name_every_slot_and_show_the_armed_one_counting_down() {
    let p = body();
    let c = ctx(&p, 10.5);
    let mut t = Transitions::new(default_slots());
    let lines = t.lines(&c);
    assert_eq!(lines[0].name, "Bass swap");
    assert_eq!(lines[0].detail, "16 bars");
    assert_eq!(lines[3].detail, "cut");
    assert_eq!(lines[8].name, "Auto");
    assert_eq!(lines[8].state, SlotState::Ready);
    t.press(BASS_SWAP, 1.0, &c);
    let lines = t.lines(&c);
    assert_eq!(lines[0].state, SlotState::Armed);
    assert_eq!(lines[0].detail, "▸ 6 bars");
    let next = t.next(&c);
    assert!(
        next.iter().any(|l| l.contains("A") && l.contains("B")),
        "{next:?}"
    );
}

#[test]
fn switching_a_slot_off_puts_back_what_its_first_tap_replaced() {
    let p = body();
    let c = ctx(&p, 10.5);
    let mut t = Transitions::new(default_slots());
    t.press(BASS_SWAP, 1.0, &c);
    t.press(JUMP_CUT, 5.0, &c);
    assert_eq!(t.press(JUMP_CUT, 5.1, &c), Press::Disabled);
    assert_eq!(t.armed_slot(), Some(BASS_SWAP));
}
