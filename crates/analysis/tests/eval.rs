//! Scoring the detectors against reference annotations, as the GiantSteps datasets have them.

use analysis::eval::{key_score, parse_key_annotation, tempo_verdict, Accuracy, TempoVerdict};
use analysis::key::{Key, Mode, PitchClass};

fn key(camelot: &str) -> Key {
    Key::from_camelot(camelot).unwrap()
}

#[test]
fn a_tempo_within_two_percent_counts_as_right() {
    assert_eq!(tempo_verdict(128.0, 128.0), TempoVerdict::Exact);
    assert_eq!(tempo_verdict(129.9, 128.0), TempoVerdict::Exact);
    assert_eq!(tempo_verdict(126.2, 128.0), TempoVerdict::Exact);
    assert_eq!(tempo_verdict(133.0, 128.0), TempoVerdict::Wrong);
}

#[test]
fn half_and_double_time_are_flagged_rather_than_failed() {
    assert_eq!(tempo_verdict(64.0, 128.0), TempoVerdict::Octave);
    assert_eq!(tempo_verdict(256.0, 128.0), TempoVerdict::Octave);
    assert_eq!(
        tempo_verdict(192.0, 128.0),
        TempoVerdict::Octave,
        "three halves"
    );
    assert_eq!(
        tempo_verdict(85.33, 128.0),
        TempoVerdict::Octave,
        "two thirds"
    );
    assert_eq!(tempo_verdict(100.0, 128.0), TempoVerdict::Wrong);
}

#[test]
fn a_tempo_that_makes_no_sense_is_wrong_not_an_octave() {
    assert_eq!(tempo_verdict(0.0, 128.0), TempoVerdict::Wrong);
    assert_eq!(tempo_verdict(128.0, 0.0), TempoVerdict::Wrong);
    assert_eq!(tempo_verdict(f64::NAN, 128.0), TempoVerdict::Wrong);
}

#[test]
fn key_scoring_follows_the_mirex_weights() {
    // 8A is A minor, 8B is C major; either way round the wheel is a fifth, so 9A is E minor
    // and 7A is D minor.
    assert_eq!(key_score(key("8A"), key("8A")), 1.0, "exact");
    assert_eq!(key_score(key("9A"), key("8A")), 0.5, "a fifth up");
    assert_eq!(key_score(key("7A"), key("8A")), 0.5, "a fifth down");
    assert_eq!(key_score(key("8B"), key("8A")), 0.3, "relative major");
    assert_eq!(key_score(key("8A"), key("8B")), 0.3, "relative minor");
    assert_eq!(
        key_score(
            Key {
                tonic: PitchClass(9),
                mode: Mode::Major
            },
            key("8A")
        ),
        0.2,
        "A major against A minor is the parallel"
    );
    assert_eq!(key_score(key("2B"), key("8A")), 0.0, "unrelated");
}

#[test]
fn annotations_parse_the_way_the_datasets_write_them() {
    assert_eq!(parse_key_annotation("A minor"), Some(key("8A")));
    assert_eq!(parse_key_annotation("  a   minor \n"), Some(key("8A")));
    assert_eq!(parse_key_annotation("C major"), Some(key("8B")));
    assert_eq!(parse_key_annotation("F# minor"), Some(key("11A")));
    assert_eq!(
        parse_key_annotation("Gb minor"),
        Some(key("11A")),
        "flats too"
    );
    assert_eq!(
        parse_key_annotation("Eb"),
        Some(key("5B")),
        "no mode means major"
    );
    assert_eq!(
        parse_key_annotation("8A"),
        Some(key("8A")),
        "or a Camelot code"
    );
    assert_eq!(parse_key_annotation("H minor"), None);
    assert_eq!(parse_key_annotation(""), None);
}

#[test]
fn accuracy_counts_what_the_spec_measures() {
    let mut acc = Accuracy::default();
    acc.add_tempo(TempoVerdict::Exact);
    acc.add_tempo(TempoVerdict::Exact);
    acc.add_tempo(TempoVerdict::Octave);
    acc.add_tempo(TempoVerdict::Wrong);
    assert_eq!(acc.tempo_tracks, 4);
    assert_eq!(acc.tempo_exact(), 0.5);
    assert_eq!(acc.tempo_within_octave(), 0.75);

    acc.add_key(1.0);
    acc.add_key(0.5);
    assert_eq!(acc.key_exact(), 0.5, "one of the two was spot on");
    assert_eq!(acc.key_weighted(), 0.75);
}

#[test]
fn an_empty_run_scores_zero_rather_than_dividing_by_nothing() {
    let acc = Accuracy::default();
    assert_eq!(acc.tempo_exact(), 0.0);
    assert_eq!(acc.key_weighted(), 0.0);
}
