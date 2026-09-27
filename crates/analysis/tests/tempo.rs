//! Tempo and beat grid (spec 3.5): constant-tempo estimate folded into a preferred range,
//! plus the time of the first downbeat-aligned beat.

mod common;
use analysis::tempo::{detect_tempo, TempoRange};
use common::*;

fn bpm_of(t: &engine::Track) -> f64 {
    detect_tempo(t, TempoRange::default()).expect("tempo").bpm
}

#[test]
fn click_tracks_at_common_tempos() {
    for bpm in [124.0, 128.0, 140.0, 174.0] {
        let got = bpm_of(&clicks(bpm, 60.0, 0.0));
        assert!((got - bpm as f64).abs() < 0.05, "{bpm}: got {got:.3}");
    }
}

#[test]
fn a_house_loop_with_off_beat_hats_reads_as_the_kick_tempo() {
    let got = bpm_of(&house(126.0, 60.0));
    assert!((got - 126.0).abs() < 0.05, "got {got:.3}");
}

#[test]
fn half_time_is_folded_into_the_default_range() {
    let got = bpm_of(&clicks(70.0, 60.0, 0.0));
    assert!((got - 140.0).abs() < 0.1, "got {got:.3}");
}

#[test]
fn the_range_is_configurable() {
    let got = detect_tempo(&clicks(70.0, 60.0, 0.0), TempoRange { min: 60.0, max: 120.0 }).unwrap().bpm;
    assert!((got - 70.0).abs() < 0.05, "got {got:.3}");
}

#[test]
fn beat_grid_lines_up_with_the_first_beat() {
    let bpm = 128.0;
    let beat = 60.0 / bpm as f64;
    let first = 0.35;
    let g = detect_tempo(&clicks(bpm, 60.0, first as f32), TempoRange::default()).unwrap();
    // The offset is reported modulo one beat.
    let err = ((g.first_beat_secs - first).rem_euclid(beat) + beat / 2.0).rem_euclid(beat) - beat / 2.0;
    assert!(err.abs() < 0.012, "offset {:.4} vs {first}, error {err:.4}", g.first_beat_secs);
    assert!(g.first_beat_secs >= 0.0 && g.first_beat_secs < beat);
}

#[test]
fn silence_has_no_tempo() {
    assert!(detect_tempo(&silence(10.0), TempoRange::default()).is_none());
}

#[test]
fn beat_position_helpers() {
    let g = analysis::tempo::BeatGrid { bpm: 120.0, first_beat_secs: 0.25 };
    assert_eq!(g.beat_secs(), 0.5);
    // 0.25 s is beat 0 of bar 1; 2.25 s is four beats later: bar 2, beat 1.
    assert_eq!(g.bar_and_beat(0.25), (1, 1));
    assert_eq!(g.bar_and_beat(2.26), (2, 1));
    assert_eq!(g.bar_and_beat(1.3), (1, 3));
    assert!((g.phase(0.5) - 0.5).abs() < 1e-9);
}
