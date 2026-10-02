//! Deck output mode: each deck leaves on its own pair, untouched by the mixer.

use engine::{Command, DeckId::*, Engine, OutputMode, Track};
use std::sync::Arc;

/// A track whose every sample is `v`, so a gain change is visible in one sample.
fn flat(v: f32) -> Arc<Track> {
    Arc::new(Track::from_interleaved(vec![v; 4_000], 48_000))
}

fn playing_both() -> Engine {
    let mut e = Engine::with_output(48_000, OutputMode::Decks);
    e.apply(Command::Load(A, flat(0.5)));
    e.apply(Command::Load(B, flat(0.25)));
    e.apply(Command::PlayPause(A));
    e.apply(Command::PlayPause(B));
    e
}

#[test]
fn deck_a_goes_to_the_first_buffer_and_deck_b_to_the_second() {
    let mut e = playing_both();
    let (mut a, mut b) = (vec![0.0; 512], vec![0.0; 512]);
    e.process(&mut a, &mut b);
    assert!(
        a.iter().all(|s| (s - 0.5).abs() < 1e-6),
        "deck A: {:?}",
        &a[..4]
    );
    assert!(
        b.iter().all(|s| (s - 0.25).abs() < 1e-6),
        "deck B: {:?}",
        &b[..4]
    );
}

#[test]
fn the_channel_strip_and_master_bus_are_bypassed() {
    let mut e = playing_both();
    e.apply(Command::SetTrim(A, 6.0));
    e.apply(Command::SetEqKill(A, engine::dsp::EqBand::Low, true));
    e.apply(Command::SetChannelFader(A, 0.0));
    e.apply(Command::SetCrossfader(1.0));
    e.apply(Command::SetFilter(-1.0));
    let (mut a, mut b) = (vec![0.0; 512], vec![0.0; 512]);
    e.process(&mut a, &mut b);
    assert!(
        a.iter().all(|s| (s - 0.5).abs() < 1e-6),
        "deck A should be untouched by trim, EQ, fader, crossfader or filter: {:?}",
        &a[..4]
    );
}

#[test]
fn silence_with_no_track_loaded() {
    let mut e = Engine::with_output(48_000, OutputMode::Decks);
    let (mut a, mut b) = (vec![9.0; 512], vec![9.0; 512]);
    e.process(&mut a, &mut b);
    assert!(a.iter().all(|s| *s == 0.0), "deck A should be silent");
    assert!(b.iter().all(|s| *s == 0.0), "deck B should be silent");
}

#[test]
fn meters_follow_each_deck_and_the_master_meter_stays_quiet() {
    let mut e = playing_both();
    let (mut a, mut b) = (vec![0.0; 512], vec![0.0; 512]);
    e.process(&mut a, &mut b);
    let m = e.take_peaks();
    assert!(
        (m.channels[0] - 0.5).abs() < 1e-6,
        "A peak: {}",
        m.channels[0]
    );
    assert!(
        (m.channels[1] - 0.25).abs() < 1e-6,
        "B peak: {}",
        m.channels[1]
    );
    assert_eq!(m.master, 0.0, "no master bus exists in deck mode");
}

#[test]
fn mix_is_the_default() {
    assert_eq!(OutputMode::default(), OutputMode::Mix);
}
