//! Key lock: the tempo fader changes speed without changing pitch.

use engine::{Deck, Track};
use std::sync::Arc;

const FS: u32 = 48_000;

fn sine_track(freq: f32, secs: f32) -> Arc<Track> {
    let frames = (secs * FS as f32) as usize;
    let data = (0..frames)
        .flat_map(|i| {
            let s = 0.4 * (std::f32::consts::TAU * freq * i as f32 / FS as f32).sin();
            [s, s]
        })
        .collect();
    Arc::new(Track::from_interleaved(data, FS))
}

fn playing(track: Arc<Track>, rate: f64, key_lock: bool) -> Deck {
    let mut deck = Deck::new();
    deck.load(track);
    deck.set_key_lock(key_lock);
    deck.set_rate(rate);
    deck.play_pause();
    deck
}

fn render(deck: &mut Deck, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * 2];
    deck.render(&mut out);
    out
}

/// Dominant frequency of the left channel, from how often it crosses zero going up.
fn pitch(buf: &[f32], skip: usize) -> f32 {
    let left: Vec<f32> = buf.iter().step_by(2).skip(skip).copied().collect();
    let crossings = left
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    crossings as f32 * FS as f32 / left.len() as f32
}

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
}

#[test]
fn without_key_lock_a_faster_rate_raises_the_pitch() {
    let mut deck = playing(sine_track(440.0, 4.0), 1.2, false);
    let out = render(&mut deck, FS as usize);
    assert!(
        (pitch(&out, 0) - 528.0).abs() < 5.0,
        "440 at 1.2x is 528: {}",
        pitch(&out, 0)
    );
}

#[test]
fn key_lock_holds_the_pitch_while_the_rate_changes() {
    for rate in [0.92, 1.08, 1.5] {
        let mut deck = playing(sine_track(440.0, 8.0), rate, true);
        let out = render(&mut deck, FS as usize);
        let heard = pitch(&out, FS as usize / 10);
        assert!(
            (heard - 440.0).abs() < 8.0,
            "rate {rate} should still sound like 440: {heard}"
        );
    }
}

#[test]
fn key_lock_still_moves_through_the_track_at_the_set_rate() {
    let mut deck = playing(sine_track(440.0, 8.0), 1.2, true);
    render(&mut deck, 10_000);
    assert!(
        (deck.position() - 12_000.0).abs() < 1.0,
        "a second of output covers 1.2 seconds of track: {}",
        deck.position()
    );
}

#[test]
fn key_lock_at_normal_speed_leaves_the_sound_alone() {
    let track = sine_track(440.0, 4.0);
    let mut plain = playing(track.clone(), 1.0, false);
    let mut locked = playing(track, 1.0, true);
    let a = render(&mut plain, 24_000);
    let b = render(&mut locked, 24_000);
    assert_eq!(a, b, "nothing to stretch, so nothing is stretched");
}

#[test]
fn key_lock_keeps_the_level_steady() {
    let mut deck = playing(sine_track(440.0, 8.0), 1.08, true);
    let out = render(&mut deck, FS as usize);
    let quarter = out.len() / 4;
    let first = rms(&out[quarter..quarter * 2]);
    let last = rms(&out[quarter * 3..]);
    assert!(first > 0.15 && last > 0.15, "{first} {last}");
    assert!(
        (first - last).abs() < 0.05,
        "the overlap-add does not pump: {first} vs {last}"
    );
    assert!(out.iter().all(|s| s.abs() <= 1.0), "and never clips");
}

#[test]
fn switching_key_lock_on_mid_play_does_not_glitch_or_jump() {
    let mut deck = playing(sine_track(440.0, 8.0), 1.08, false);
    render(&mut deck, 12_000);
    let before = deck.position();
    deck.set_key_lock(true);
    let out = render(&mut deck, 12_000);
    assert!(out.iter().all(|s| s.is_finite()));
    assert!(
        (deck.position() - before - 12_000.0 * 1.08).abs() < 1.0,
        "the playhead carries on where it was"
    );
    assert!(deck.key_lock(), "and the deck says it is on");
}

#[test]
fn a_seek_under_key_lock_lands_where_it_was_told() {
    let mut deck = playing(sine_track(440.0, 8.0), 1.08, true);
    render(&mut deck, 4_800);
    deck.seek(240_000.0);
    let out = render(&mut deck, 4_800);
    assert!(out.iter().all(|s| s.is_finite()));
    assert!((deck.position() - (240_000.0 + 4_800.0 * 1.08)).abs() < 1.0);
}
