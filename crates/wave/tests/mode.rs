use wave::WaveformMode;

#[test]
fn the_default_is_three_band() {
    assert_eq!(WaveformMode::default(), WaveformMode::ThreeBand);
}

#[test]
fn cycling_visits_every_mode_and_returns() {
    let m = WaveformMode::ThreeBand;
    assert_eq!(m.next(), WaveformMode::Rgb);
    assert_eq!(m.next().next(), WaveformMode::Blue);
    assert_eq!(m.next().next().next(), WaveformMode::ThreeBand);
}

#[test]
fn modes_can_key_a_hash_map() {
    // pixel.rs puts the mode in its redraw fingerprint.
    use std::collections::HashSet;
    let set: HashSet<_> = [WaveformMode::ThreeBand, WaveformMode::Rgb].into();
    assert!(set.contains(&WaveformMode::ThreeBand));
    assert!(!set.contains(&WaveformMode::Blue));
}
