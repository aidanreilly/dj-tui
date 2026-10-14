//! The transition planner: where a mix starts, where the next track comes in, and what the
//! EQ and faders do on the way. Pure numbers in, a plan out.

use analysis::key::Key;
use analysis::phrase::{Phrases, Section, SectionKind};
use analysis::tempo::BeatGrid;
use dj_tui::automation::Curve;
use dj_tui::transition::{plan, Context, DeckInfo, Kind, Plan, Slot, StepAction};
use engine::DeckId::{A, B};
use input::{Band, Control};

const SR: u32 = 48_000;
/// 120 BPM: half a second a beat.
const BEAT: f64 = 24_000.0;
const BAR: f64 = 4.0 * BEAT;

fn phrases(sections: &[(SectionKind, usize, usize)]) -> Phrases {
    let bars = sections.iter().map(|s| s.1 + s.2).max().unwrap_or(128);
    Phrases {
        downbeat_beat: 0,
        bar_levels: vec![[1.0; 3]; bars],
        sections: sections
            .iter()
            .map(|&(kind, start_bar, bars)| Section {
                kind,
                start_bar,
                bars,
            })
            .collect(),
    }
}

fn all_body() -> Phrases {
    phrases(&[(SectionKind::Body, 0, 128)])
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

fn playing_at<'a>(p: &'a Phrases, bar: f64) -> DeckInfo<'a> {
    DeckInfo {
        playing: true,
        position: bar * BAR,
        ..deck(p)
    }
}

fn ctx<'a>(a: DeckInfo<'a>, b: DeckInfo<'a>) -> Context<'a> {
    Context {
        decks: [a, b],
        crossfader: 0.0,
        sample_rate: SR,
        tempo_range: 0.08,
    }
}

fn slot(kind: Kind) -> Slot {
    Slot { kind, bars: None }
}

fn ok(kind: Kind, c: &Context) -> Plan {
    plan(&slot(kind), c).unwrap_or_else(|e| panic!("{kind:?} refused: {e}"))
}

fn fires(p: &Plan, at: f64, action: StepAction) -> bool {
    p.steps
        .iter()
        .any(|s| (s.at - at).abs() < 1e-6 && s.action == action)
}

fn set(control: Control, to: f32) -> StepAction {
    StepAction::Set { control, to }
}

fn fades(p: &Plan, control: Control) -> bool {
    p.steps
        .iter()
        .any(|s| matches!(s.action, StepAction::Fade { control: c, .. } if c == control))
}

fn key(code: &str) -> Option<Key> {
    Key::from_camelot(code)
}

#[test]
fn nothing_playing_means_nothing_to_mix_out_of() {
    let p = all_body();
    let err = plan(&slot(Kind::BassSwap), &ctx(deck(&p), deck(&p))).unwrap_err();
    assert!(err.contains("Nothing is playing"), "{err}");
}

#[test]
fn an_empty_other_deck_has_nothing_to_bring_in() {
    let p = all_body();
    let b = DeckInfo {
        loaded: false,
        ..deck(&p)
    };
    let err = plan(&slot(Kind::BassSwap), &ctx(playing_at(&p, 10.0), b)).unwrap_err();
    assert!(err.contains("Deck B is empty"), "{err}");
}

#[test]
fn the_playing_deck_goes_out_and_the_other_comes_in() {
    let p = all_body();
    let plan = ok(Kind::BassSwap, &ctx(deck(&p), playing_at(&p, 10.0)));
    assert_eq!((plan.out_deck, plan.in_deck), (B, A));
}

#[test]
fn with_both_playing_the_one_on_air_goes_out() {
    let p = all_body();
    let mut c = ctx(playing_at(&p, 10.0), playing_at(&p, 10.0));
    c.crossfader = 0.8;
    assert_eq!(ok(Kind::BassSwap, &c).out_deck, B);
    c.crossfader = 0.0;
    c.decks[0].fader = 0.2;
    assert_eq!(ok(Kind::BassSwap, &c).out_deck, B);
}

#[test]
fn a_bass_swap_starts_on_the_next_eight_bar_line_and_swaps_lows_halfway() {
    let p = all_body();
    let mut b = deck(&p);
    b.hot_cues[0] = Some(4.0 * BAR);
    let plan = ok(Kind::BassSwap, &ctx(playing_at(&p, 10.5), b));
    assert_eq!(plan.start, 16.0 * BAR);
    assert_eq!(plan.length_beats, 64.0);
    assert_eq!(plan.in_from, Some(4.0 * BAR));
    assert_eq!(plan.in_label, "cue 1");
    assert!(fires(&plan, -1.0, set(Control::Eq(B, Band::Low), 0.0)));
    assert!(fires(&plan, 0.0, StepAction::StartIncoming));
    assert!(fires(&plan, 32.0, set(Control::Eq(A, Band::Low), 0.0)));
    assert!(fires(&plan, 32.0, set(Control::Eq(B, Band::Low), 0.5)));
    assert!(fires(&plan, 64.0, StepAction::StopOutgoing));
}

#[test]
fn the_length_can_be_set_per_slot() {
    let p = all_body();
    let c = ctx(playing_at(&p, 10.5), deck(&p));
    let plan = plan(
        &Slot {
            kind: Kind::BassSwap,
            bars: Some(8),
        },
        &c,
    )
    .unwrap();
    assert_eq!(plan.length_beats, 32.0);
}

#[test]
fn without_a_hot_cue_the_main_cue_then_the_first_bar_is_the_way_in() {
    let p = all_body();
    let mut b = deck(&p);
    b.cue_point = 2.0 * BAR;
    let plan1 = ok(Kind::BassSwap, &ctx(playing_at(&p, 10.5), b));
    assert_eq!(plan1.in_from, Some(2.0 * BAR));
    assert_eq!(plan1.in_label, "cue");

    let late = Phrases {
        downbeat_beat: 1,
        ..all_body()
    };
    let plan2 = ok(Kind::BassSwap, &ctx(playing_at(&p, 10.5), deck(&late)));
    assert_eq!(plan2.in_from, Some(BEAT), "bar 1 is on the second beat");
    assert_eq!(plan2.in_label, "bar 1");
}

#[test]
fn a_hot_cue_ahead_in_the_back_half_is_taken_as_the_mix_out_point() {
    let p = all_body();
    let mut a = playing_at(&p, 60.0);
    a.hot_cues[0] = Some(20.0 * BAR);
    a.hot_cues[2] = Some(70.0 * BAR);
    let plan = ok(Kind::BassSwap, &ctx(a, deck(&p)));
    assert_eq!(plan.start, 70.0 * BAR);
    assert_eq!(plan.out_label, "cue 3");
}

#[test]
fn a_section_change_close_ahead_beats_the_plain_phrase_line() {
    let p = phrases(&[
        (SectionKind::Body, 0, 74),
        (SectionKind::Breakdown, 74, 16),
        (SectionKind::Drop, 90, 38),
    ]);
    let plan = ok(
        Kind::BassSwap,
        &ctx(playing_at(&p, 60.0), deck(&all_body())),
    );
    assert_eq!(plan.start, 74.0 * BAR);
    assert_eq!(plan.out_label, "breakdown");
}

#[test]
fn the_incoming_deck_is_brought_to_the_outgoing_tempo_folding_half_time() {
    let p = all_body();
    let mut a = playing_at(&p, 10.0);
    a.rate = 1.02;
    let mut b = deck(&p);
    b.grid = Some(BeatGrid {
        bpm: 64.0,
        first_beat_secs: 0.0,
    });
    let plan = ok(Kind::BassSwap, &ctx(a, b));
    let rate = plan.in_rate.unwrap();
    assert!((rate - 122.4 / 128.0).abs() < 1e-9, "{rate}");
}

#[test]
fn a_tempo_gap_the_fader_cannot_close_turns_a_blend_into_a_cut() {
    let p = all_body();
    let mut b = deck(&p);
    b.grid = Some(BeatGrid {
        bpm: 140.0,
        first_beat_secs: 0.0,
    });
    let plan = ok(Kind::LongBlend, &ctx(playing_at(&p, 10.0), b));
    assert!(
        plan.notes.iter().any(|n| n.contains("tempo")),
        "{:?}",
        plan.notes
    );
    assert!(!fades(&plan, Control::Fader(B)));
    assert!(fires(&plan, 0.0, set(Control::Fader(B), 1.0)));
}

#[test]
fn without_a_grid_a_blend_becomes_a_fade_by_the_clock() {
    let p = all_body();
    let mut a = playing_at(&p, 10.0);
    a.grid = None;
    let plan = ok(Kind::BassSwap, &ctx(a, deck(&p)));
    assert!(
        plan.notes.iter().any(|n| n.contains("no grid")),
        "{:?}",
        plan.notes
    );
    assert!(plan.start > a.position && plan.start < a.position + SR as f64 * 2.0);
    assert!(fades(&plan, Control::Fader(B)));
}

#[test]
fn a_blend_that_would_run_past_the_end_is_shortened() {
    let p = all_body();
    let plan = ok(Kind::LongBlend, &ctx(playing_at(&p, 120.2), deck(&p)));
    assert_eq!(plan.start, 122.0 * BAR);
    assert_eq!(plan.length_beats, 16.0);
    assert!(
        plan.notes.iter().any(|n| n.contains("runway")),
        "{:?}",
        plan.notes
    );
}

#[test]
fn a_jump_cut_lands_on_the_next_four_bar_line_in_one_go() {
    let p = all_body();
    let plan = ok(Kind::JumpCut, &ctx(playing_at(&p, 10.5), deck(&p)));
    assert_eq!(plan.start, 12.0 * BAR);
    assert_eq!(plan.length_beats, 0.0);
    assert!(fires(&plan, 0.0, StepAction::StartIncoming));
    assert!(fires(&plan, 0.0, set(Control::Fader(A), 0.0)));
    assert!(fires(&plan, 0.0, set(Control::Fader(B), 1.0)));
    assert!(fires(&plan, 0.0, StepAction::StopOutgoing));
    assert!(!fades(&plan, Control::Fader(A)));
}

#[test]
fn a_drop_swap_cuts_where_the_outgoing_breakdown_ends_into_the_incoming_drop() {
    let a = phrases(&[
        (SectionKind::Body, 0, 48),
        (SectionKind::Breakdown, 48, 16),
        (SectionKind::Drop, 64, 64),
    ]);
    let b = phrases(&[(SectionKind::Intro, 0, 16), (SectionKind::Body, 16, 112)]);
    let plan = ok(Kind::DropSwap, &ctx(playing_at(&a, 50.0), deck(&b)));
    assert_eq!(plan.start, 64.0 * BAR);
    assert_eq!(plan.in_from, Some(16.0 * BAR));
    assert_eq!(plan.in_label, "drop");
    assert!(fires(&plan, 0.0, StepAction::StartIncoming));
}

#[test]
fn a_drop_swap_with_no_breakdown_ahead_cuts_on_a_sixteen_bar_line() {
    let p = all_body();
    let plan = ok(Kind::DropSwap, &ctx(playing_at(&p, 10.0), deck(&p)));
    assert_eq!(plan.start, 16.0 * BAR);
    assert!(
        plan.notes.iter().any(|n| n.contains("breakdown")),
        "{:?}",
        plan.notes
    );
}

#[test]
fn clashing_keys_hand_the_mids_over_at_once_instead_of_blending_them() {
    let p = all_body();
    let mut a = playing_at(&p, 10.0);
    a.key = key("8A");
    let mut b = deck(&p);
    b.key = key("9A");
    let blend = ok(Kind::LongBlend, &ctx(a, b));
    assert!(blend.notes.is_empty(), "{:?}", blend.notes);
    assert!(fades(&blend, Control::Eq(A, Band::Mid)));

    b.key = key("3B");
    let clash = ok(Kind::LongBlend, &ctx(a, b));
    assert!(
        clash.notes.iter().any(|n| n.contains("keys clash")),
        "{:?}",
        clash.notes
    );
    assert!(fires(&clash, 64.0, set(Control::Eq(A, Band::Mid), 0.0)));
    assert!(!fades(&clash, Control::Eq(A, Band::Mid)));
}

#[test]
fn pitching_a_track_moves_its_key_unless_key_lock_holds_it() {
    let p = all_body();
    let mut a = playing_at(&p, 10.0);
    a.key = key("8A");
    a.grid = Some(BeatGrid {
        bpm: 124.5,
        first_beat_secs: 0.0,
    });
    let mut b = deck(&p);
    b.key = key("8A");
    // B has to come up 3.75 %, which is more than half a semitone.
    let shifted = ok(Kind::LongBlend, &ctx(a, b));
    assert!(
        shifted.notes.iter().any(|n| n.contains("keys clash")),
        "{:?}",
        shifted.notes
    );
    b.key_lock = true;
    let held = ok(Kind::LongBlend, &ctx(a, b));
    assert!(held.notes.is_empty(), "{:?}", held.notes);
}

#[test]
fn with_the_crossfader_parked_on_the_outgoing_side_the_crossfader_does_the_handover() {
    let p = all_body();
    let mut c = ctx(playing_at(&p, 10.0), deck(&p));
    c.crossfader = -1.0;
    let plan = ok(Kind::BassSwap, &c);
    assert!(fades(&plan, Control::Crossfader));
    assert!(!fades(&plan, Control::Fader(B)));
    c.crossfader = 0.0;
    let plan = ok(Kind::BassSwap, &c);
    assert!(!fades(&plan, Control::Crossfader));
    assert!(fades(&plan, Control::Fader(B)));
    assert!(fires(&plan, -1.0, set(Control::Fader(B), 0.0)));
}

#[test]
fn every_kind_stops_the_outgoing_deck_and_flattens_both_eqs_at_the_end() {
    let a = phrases(&[
        (SectionKind::Body, 0, 48),
        (SectionKind::Breakdown, 48, 16),
        (SectionKind::Drop, 64, 64),
    ]);
    let b = all_body();
    let c = ctx(playing_at(&a, 40.0), deck(&b));
    for kind in Kind::ALL {
        let plan = ok(kind, &c);
        let end = plan.end_beats();
        assert!(fires(&plan, end, StepAction::StopOutgoing), "{kind:?}");
        for band in [Band::Low, Band::Mid, Band::High] {
            assert!(
                fires(&plan, end, set(Control::Eq(A, band), 0.5)),
                "{kind:?} {band:?}"
            );
            assert!(
                fires(&plan, end, set(Control::Eq(B, band), 0.5)),
                "{kind:?} {band:?}"
            );
        }
    }
}

#[test]
fn an_incoming_deck_already_playing_is_left_where_it_is() {
    let p = all_body();
    let mut c = ctx(playing_at(&p, 10.0), playing_at(&p, 3.0));
    c.decks[1].fader = 0.0;
    let plan = ok(Kind::BassSwap, &c);
    assert_eq!(plan.in_from, None);
    assert_eq!(plan.in_label, "where it is");
}

#[test]
fn a_loop_out_holds_the_outgoing_bars_until_the_blend_is_done() {
    let p = all_body();
    let plan = ok(Kind::LoopOut, &ctx(playing_at(&p, 10.0), deck(&p)));
    assert!(fires(&plan, 0.0, StepAction::LoopOutgoing { beats: 16.0 }));
    assert!(fires(&plan, plan.end_beats(), StepAction::EndLoopOutgoing));
}

#[test]
fn a_tempo_bridge_rides_the_outgoing_tempo_over_before_the_blend() {
    let p = all_body();
    let mut b = deck(&p);
    b.grid = Some(BeatGrid {
        bpm: 132.0,
        first_beat_secs: 0.0,
    });
    let plan = ok(Kind::TempoBridge, &ctx(playing_at(&p, 10.0), b));
    assert!(fires(
        &plan,
        -32.0,
        StepAction::KeyLock { deck: A, on: true }
    ));
    let ride = plan
        .steps
        .iter()
        .find(|s| {
            matches!(
                s.action,
                StepAction::Fade {
                    control: Control::Tempo(A),
                    ..
                }
            )
        })
        .expect("a tempo ride on A");
    assert_eq!(ride.at, -32.0);
    // A goes to the top of its range, 129.6, and B comes down the rest of the way.
    let rate = plan.in_rate.unwrap();
    assert!((rate - 129.6 / 132.0).abs() < 1e-9, "{rate}");
    assert!(plan.start >= 10.0 * BAR + 32.0 * BEAT);
}

#[test]
fn a_brake_winds_the_outgoing_deck_down_into_the_downbeat() {
    let p = all_body();
    let plan = ok(Kind::Brake, &ctx(playing_at(&p, 10.5), deck(&p)));
    assert!(plan
        .steps
        .iter()
        .any(|s| s.at == -2.0 && matches!(s.action, StepAction::RateRamp { deck: A, .. })));
    assert!(fires(&plan, 0.0, StepAction::StartIncoming));
    assert!(fires(&plan, 0.0, StepAction::RestoreRate { deck: A }));
}

#[test]
fn equal_power_rides_the_two_channel_faders() {
    let p = all_body();
    let plan = ok(Kind::BassSwap, &ctx(playing_at(&p, 10.0), deck(&p)));
    assert!(plan.steps.iter().any(|s| matches!(
        s.action,
        StepAction::Fade {
            control: Control::Fader(B),
            curve: Curve::EqualPower,
            ..
        }
    )));
}

mod auto {
    use super::*;

    #[test]
    fn short_runway_loops_the_outgoing_track() {
        let p = all_body();
        let plan = ok(Kind::Auto, &ctx(playing_at(&p, 118.0), deck(&p)));
        assert_eq!(plan.kind, Kind::LoopOut);
        assert!(plan.auto);
    }

    #[test]
    fn a_breakdown_coming_up_and_a_drop_to_land_on_gives_a_drop_swap() {
        let a = phrases(&[
            (SectionKind::Body, 0, 48),
            (SectionKind::Breakdown, 48, 16),
            (SectionKind::Drop, 64, 64),
        ]);
        let b = phrases(&[(SectionKind::Intro, 0, 16), (SectionKind::Body, 16, 112)]);
        let plan = ok(Kind::Auto, &ctx(playing_at(&a, 40.0), deck(&b)));
        assert_eq!(plan.kind, Kind::DropSwap);
    }

    #[test]
    fn matching_keys_with_room_to_spare_get_the_long_blend() {
        let p = all_body();
        let mut a = playing_at(&p, 10.0);
        a.key = key("8A");
        let mut b = deck(&p);
        b.key = key("8B");
        assert_eq!(ok(Kind::Auto, &ctx(a, b)).kind, Kind::LongBlend);
    }

    #[test]
    fn clashing_keys_get_a_bass_swap() {
        let p = all_body();
        let mut a = playing_at(&p, 10.0);
        a.key = key("8A");
        let mut b = deck(&p);
        b.key = key("2B");
        assert_eq!(ok(Kind::Auto, &ctx(a, b)).kind, Kind::BassSwap);
    }

    #[test]
    fn a_tempo_gap_within_reach_of_both_faders_gets_a_bridge() {
        let p = all_body();
        let mut b = deck(&p);
        b.grid = Some(BeatGrid {
            bpm: 132.0,
            first_beat_secs: 0.0,
        });
        assert_eq!(
            ok(Kind::Auto, &ctx(playing_at(&p, 10.0), b)).kind,
            Kind::TempoBridge
        );
        b.grid = Some(BeatGrid {
            bpm: 160.0,
            first_beat_secs: 0.0,
        });
        assert_eq!(
            ok(Kind::Auto, &ctx(playing_at(&p, 10.0), b)).kind,
            Kind::JumpCut
        );
    }
}

#[test]
fn kinds_read_and_write_their_config_names() {
    for kind in Kind::ALL {
        assert_eq!(Kind::parse(kind.config_name()), Some(kind));
    }
    assert_eq!(Kind::parse("bass-swap"), Some(Kind::BassSwap));
    assert_eq!(Kind::parse("nonsense"), None);
    assert_eq!(Kind::ALL.len(), 9, "eight kinds plus auto fill the row");
}
