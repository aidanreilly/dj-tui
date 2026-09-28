//! Fades that run on their own once started. A keyboard cannot sweep a control, so it starts
//! a sweep instead. Everything here is UI-thread logic driven by a frame delta.

use dj_tui::automation::{
    Automation, Curve, MAX_FADE_BEATS, MIN_FADE_BEATS, MIN_FADE_SECS, NO_GRID_BEAT_SECS,
};
use engine::DeckId::{A, B};
use input::{Control, Dir};

/// Run a fade to completion in `steps` even ticks and return every value it passed through.
fn sweep(a: &mut Automation, secs: f64, steps: usize) -> Vec<f32> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for _ in 0..steps {
        out.clear();
        a.tick(secs / steps as f64, &mut out);
        seen.extend(out.iter().map(|&(_, v)| v));
    }
    seen
}

#[test]
fn a_position_fade_moves_linearly_and_lands_on_the_target() {
    let mut a = Automation::new(8.0);
    a.start(Control::Crossfader, -1.0, 1.0, 1.0, Curve::Position);
    let seen = sweep(&mut a, 1.0, 4);
    assert_eq!(seen.len(), 4);
    assert!((seen[0] - -0.5).abs() < 1e-5, "got {}", seen[0]);
    assert!((seen[1] - 0.0).abs() < 1e-5);
    assert!((seen[2] - 0.5).abs() < 1e-5);
    assert_eq!(seen[3], 1.0, "exactly the target on the last tick");
}

#[test]
fn a_decibel_fade_out_is_even_in_decibels() {
    let mut a = Automation::new(8.0);
    a.start(Control::Fader(A), 1.0, 0.0, 1.0, Curve::Decibel);
    let seen = sweep(&mut a, 1.0, 4);
    // A -60 dB floor in four steps is -15, -30 and -45 dB, then exactly zero.
    assert!((seen[0] - 0.1778).abs() < 1e-3, "got {}", seen[0]);
    assert!((seen[1] - 0.0316).abs() < 1e-3, "got {}", seen[1]);
    assert!((seen[2] - 0.0056).abs() < 1e-3, "got {}", seen[2]);
    assert_eq!(seen[3], 0.0, "silence, not a -60 dB tail");
}

#[test]
fn a_decibel_fade_in_starts_from_the_floor() {
    let mut a = Automation::new(8.0);
    a.start(Control::Fader(A), 0.0, 1.0, 1.0, Curve::Decibel);
    let seen = sweep(&mut a, 1.0, 4);
    assert!(
        seen[0] > 0.0 && seen[0] < 0.01,
        "quiet, not silent: {}",
        seen[0]
    );
    assert_eq!(seen[3], 1.0);
}

/// Review Focus 1. A track load can block a frame; the fade must not overshoot.
#[test]
fn a_long_frame_clamps_instead_of_overshooting() {
    let mut a = Automation::new(8.0);
    a.start(Control::Crossfader, 0.0, 1.0, 1.0, Curve::Position);
    let mut out = Vec::new();
    a.tick(5.0, &mut out);
    assert_eq!(out, vec![(Control::Crossfader, 1.0)]);
    out.clear();
    a.tick(0.1, &mut out);
    assert!(out.is_empty(), "it finished, so it stopped reporting");
}

#[test]
fn a_fade_with_equal_endpoints_never_starts() {
    let mut a = Automation::new(8.0);
    a.start(Control::Crossfader, 0.5, 0.5, 1.0, Curve::Position);
    let mut out = Vec::new();
    a.tick(0.1, &mut out);
    assert!(out.is_empty());
    assert_eq!(a.target(Control::Crossfader), None);
}

#[test]
fn several_fades_run_at_once() {
    let mut a = Automation::new(8.0);
    a.start(Control::Crossfader, -1.0, 0.0, 1.0, Curve::Position);
    a.start(Control::Fader(A), 1.0, 0.0, 1.0, Curve::Decibel);
    a.start(Control::Fader(B), 0.0, 1.0, 1.0, Curve::Decibel);
    let mut out = Vec::new();
    a.tick(0.5, &mut out);
    assert_eq!(out.len(), 3);
}

#[test]
fn a_new_fade_replaces_the_one_on_that_control() {
    let mut a = Automation::new(8.0);
    a.start(Control::Crossfader, -1.0, 1.0, 10.0, Curve::Position);
    a.start(Control::Crossfader, 0.0, -1.0, 1.0, Curve::Position);
    assert_eq!(a.target(Control::Crossfader), Some(-1.0));
    let mut out = Vec::new();
    a.tick(1.0, &mut out);
    assert_eq!(out, vec![(Control::Crossfader, -1.0)]);
}

#[test]
fn cancelling_stops_a_fade_where_it_stands() {
    let mut a = Automation::new(8.0);
    a.start(Control::Crossfader, -1.0, 1.0, 1.0, Curve::Position);
    let mut out = Vec::new();
    a.tick(0.25, &mut out);
    a.cancel(Control::Crossfader);
    out.clear();
    a.tick(0.25, &mut out);
    assert!(out.is_empty());
    assert_eq!(a.target(Control::Crossfader), None);
}

#[test]
fn a_deck_cancel_leaves_the_other_deck_the_crossfader_and_the_master_filter_alone() {
    let mut a = Automation::new(8.0);
    a.start(Control::Fader(A), 1.0, 0.0, 1.0, Curve::Decibel);
    a.start(Control::Filter, 0.0, 1.0, 1.0, Curve::Position);
    a.start(Control::Fader(B), 0.0, 1.0, 1.0, Curve::Decibel);
    a.start(Control::Crossfader, -1.0, 1.0, 1.0, Curve::Position);
    a.cancel_deck(A);
    assert_eq!(a.target(Control::Fader(A)), None);
    assert_eq!(
        a.target(Control::Filter),
        Some(1.0),
        "the filter is a master control, not one deck's"
    );
    assert_eq!(a.target(Control::Fader(B)), Some(1.0));
    assert_eq!(a.target(Control::Crossfader), Some(1.0));
}

#[test]
fn it_counts_what_is_running() {
    let mut a = Automation::new(8.0);
    assert_eq!(a.running(), 0);
    a.start(Control::Crossfader, -1.0, 1.0, 1.0, Curve::Position);
    a.start(Control::Fader(A), 1.0, 0.0, 1.0, Curve::Decibel);
    assert_eq!(a.running(), 2);
    let mut out = Vec::new();
    a.tick(1.0, &mut out);
    assert_eq!(a.running(), 0, "both finished and were dropped");
}

#[test]
fn cancel_all_empties_it() {
    let mut a = Automation::new(8.0);
    a.start(Control::Crossfader, -1.0, 1.0, 1.0, Curve::Position);
    a.start(Control::Fader(A), 1.0, 0.0, 1.0, Curve::Decibel);
    a.cancel_all();
    let mut out = Vec::new();
    a.tick(1.0, &mut out);
    assert!(out.is_empty());
}

#[test]
fn the_length_halves_and_doubles_within_the_range() {
    let mut a = Automation::new(8.0);
    a.scale_length(Dir::Down);
    assert_eq!(a.fade_beats(), 4.0);
    a.scale_length(Dir::Up);
    a.scale_length(Dir::Up);
    assert_eq!(a.fade_beats(), 16.0);
    for _ in 0..8 {
        a.scale_length(Dir::Down);
    }
    assert_eq!(a.fade_beats(), MIN_FADE_BEATS);
    for _ in 0..12 {
        a.scale_length(Dir::Up);
    }
    assert_eq!(a.fade_beats(), MAX_FADE_BEATS);
}

#[test]
fn a_length_in_seconds_comes_from_the_beat_grid_and_the_rate() {
    let a = Automation::new(8.0);
    // 120 BPM at 48 kHz is 24000 frames a beat, so eight beats is four seconds.
    let secs = a.secs_for(Some(24_000.0), 1.0, 48_000);
    assert!((secs - 4.0).abs() < 1e-9, "got {secs}");
    // A deck running fast has shorter beats.
    let faster = a.secs_for(Some(24_000.0), 1.08, 48_000);
    assert!((faster - 4.0 / 1.08).abs() < 1e-9, "got {faster}");
}

#[test]
fn a_deck_with_no_grid_counts_a_beat_as_half_a_second() {
    let a = Automation::new(8.0);
    let secs = a.secs_for(None, 1.0, 48_000);
    assert!((secs - 8.0 * NO_GRID_BEAT_SECS).abs() < 1e-9, "got {secs}");
}

#[test]
fn a_very_short_length_is_held_to_the_floor() {
    let mut a = Automation::new(8.0);
    for _ in 0..8 {
        a.scale_length(Dir::Down);
    }
    // Two beats at 200 BPM is 0.6 s, under the floor that keeps the steps small.
    let secs = a.secs_for(Some(14_400.0), 1.0, 48_000);
    assert!((secs - MIN_FADE_SECS).abs() < 1e-9, "got {secs}");
}
