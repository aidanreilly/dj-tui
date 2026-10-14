//! Sections and phrase lines read from the band envelope the loader already keeps.

use analysis::phrase::{detect_phrases, SectionKind};
use analysis::tempo::BeatGrid;

const BPM: f64 = 120.0;
/// Two seconds a bar at 120 BPM.
const BAR_SECS: f64 = 2.0;
const POINTS_PER_BAR: usize = 16;

fn grid() -> BeatGrid {
    BeatGrid {
        bpm: BPM,
        first_beat_secs: 0.0,
    }
}

/// An envelope built bar by bar: `true` is a bar with the kick in, `false` one without.
/// `shift_beats` moves every change that many beats later, as a track whose grid starts on
/// the wrong beat of the bar would look.
fn envelope(bars: &[bool], shift_beats: usize) -> (Vec<[f32; 3]>, f64) {
    let per_beat = POINTS_PER_BAR / 4;
    let mut points = vec![[0.05, 0.3, 0.3]; shift_beats * per_beat];
    for &full in bars {
        let low = if full { 0.9 } else { 0.05 };
        points.extend(std::iter::repeat_n([low, 0.4, 0.3], POINTS_PER_BAR));
    }
    let secs = points.len() as f64 * BAR_SECS / POINTS_PER_BAR as f64;
    (points, secs)
}

fn pattern(spec: &[(bool, usize)]) -> Vec<bool> {
    spec.iter()
        .flat_map(|&(full, n)| std::iter::repeat_n(full, n))
        .collect()
}

#[test]
fn a_dance_track_splits_into_intro_body_breakdown_drop_and_outro() {
    let bars = pattern(&[
        (false, 16),
        (true, 32),
        (false, 16),
        (true, 32),
        (false, 16),
    ]);
    let (env, secs) = envelope(&bars, 0);
    let p = detect_phrases(&env, secs, &grid());
    let kinds: Vec<(SectionKind, usize, usize)> = p
        .sections
        .iter()
        .map(|s| (s.kind, s.start_bar, s.bars))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (SectionKind::Intro, 0, 16),
            (SectionKind::Body, 16, 32),
            (SectionKind::Breakdown, 48, 16),
            (SectionKind::Drop, 64, 32),
            (SectionKind::Outro, 96, 16),
        ]
    );
}

#[test]
fn a_track_with_no_kick_at_all_is_one_section() {
    let (env, secs) = envelope(&pattern(&[(false, 64)]), 0);
    let p = detect_phrases(&env, secs, &grid());
    assert_eq!(p.sections.len(), 1);
    assert_eq!(p.sections[0].bars, 64);
}

#[test]
fn a_two_bar_dropout_is_not_a_breakdown() {
    let bars = pattern(&[(true, 30), (false, 2), (true, 32)]);
    let (env, secs) = envelope(&bars, 0);
    let p = detect_phrases(&env, secs, &grid());
    assert!(
        p.sections.iter().all(|s| s.kind != SectionKind::Breakdown),
        "{:?}",
        p.sections
    );
}

#[test]
fn the_downbeat_is_found_when_the_grid_starts_on_the_wrong_beat() {
    let bars = pattern(&[(false, 16), (true, 32), (false, 16)]);
    let (env, secs) = envelope(&bars, 1);
    let p = detect_phrases(&env, secs, &grid());
    assert_eq!(p.downbeat_beat, 1);
    // Bar lines are counted from that beat, so the body still starts on bar 16.
    let body = p
        .sections
        .iter()
        .find(|s| s.kind == SectionKind::Body)
        .unwrap();
    assert_eq!(body.start_bar, 16);
    assert!((p.bar_secs(&grid(), body.start_bar) - (16.0 * BAR_SECS + 0.5)).abs() < 1e-9);
}

#[test]
fn bar_positions_go_both_ways() {
    let (env, secs) = envelope(&pattern(&[(true, 64)]), 0);
    let p = detect_phrases(&env, secs, &grid());
    assert!((p.bar_secs(&grid(), 8) - 16.0).abs() < 1e-9);
    assert!((p.bar_at(&grid(), 17.0) - 8.5).abs() < 1e-9);
    assert_eq!(p.bar_count(), 64);
}

#[test]
fn sections_can_be_looked_up_by_bar() {
    let bars = pattern(&[(false, 16), (true, 48)]);
    let (env, secs) = envelope(&bars, 0);
    let p = detect_phrases(&env, secs, &grid());
    assert_eq!(p.section_at(3).map(|s| s.kind), Some(SectionKind::Intro));
    assert_eq!(p.section_at(20).map(|s| s.kind), Some(SectionKind::Body));
    assert_eq!(p.section_at(999), None);
}

#[test]
fn an_empty_envelope_gives_nothing_rather_than_a_panic() {
    let p = detect_phrases(&[], 0.0, &grid());
    assert!(p.sections.is_empty());
    assert_eq!(p.bar_count(), 0);
}

#[test]
fn the_first_drop_falls_back_to_the_first_body() {
    let bars = pattern(&[(false, 8), (true, 56)]);
    let (env, secs) = envelope(&bars, 0);
    let p = detect_phrases(&env, secs, &grid());
    assert_eq!(p.first_drop(), Some(8));
    let bars = pattern(&[(false, 8), (true, 16), (false, 8), (true, 32)]);
    let (env, secs) = envelope(&bars, 0);
    let p = detect_phrases(&env, secs, &grid());
    assert_eq!(p.first_drop(), Some(32));
}

#[test]
fn the_outro_start_is_where_the_kick_goes_for_good() {
    let bars = pattern(&[(true, 48), (false, 16)]);
    let (env, secs) = envelope(&bars, 0);
    let p = detect_phrases(&env, secs, &grid());
    assert_eq!(p.outro_start(), Some(48));
    let (env, secs) = envelope(&pattern(&[(true, 64)]), 0);
    assert_eq!(detect_phrases(&env, secs, &grid()).outro_start(), None);
}
