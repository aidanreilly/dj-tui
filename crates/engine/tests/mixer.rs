//! Crossfader curves and bus routing through the engine.

use engine::{crossfader_gains, CrossfaderCurve, DeckId, Engine, Track};
use std::sync::Arc;

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn dc_track(value: f32, frames: usize) -> Arc<Track> {
    Arc::new(Track::from_interleaved(vec![value; frames * 2], 48_000))
}

// --- Crossfader curves ---

#[test]
fn linear_curve_is_half_each_at_centre_and_full_at_ends() {
    let c = CrossfaderCurve::Linear;
    let (a, b) = crossfader_gains(0.0, c);
    assert!(approx(a, 0.5) && approx(b, 0.5));
    assert_eq!(crossfader_gains(-1.0, c), (1.0, 0.0));
    assert_eq!(crossfader_gains(1.0, c), (0.0, 1.0));
}

#[test]
fn constant_power_curve_keeps_summed_power_at_unity() {
    let c = CrossfaderCurve::ConstantPower;
    for i in -10..=10 {
        let (a, b) = crossfader_gains(i as f32 / 10.0, c);
        assert!(approx(a * a + b * b, 1.0), "at {i}: {a} {b}");
    }
    let (a, b) = crossfader_gains(0.0, c);
    assert!(approx(a, std::f32::consts::FRAC_1_SQRT_2) && approx(b, a));
}

#[test]
fn cut_curve_keeps_both_full_until_the_last_stretch() {
    let c = CrossfaderCurve::Cut;
    assert_eq!(crossfader_gains(0.0, c), (1.0, 1.0));
    assert_eq!(crossfader_gains(-1.0, c), (1.0, 0.0));
    assert_eq!(crossfader_gains(1.0, c), (0.0, 1.0));
    let (a, _) = crossfader_gains(0.99, c);
    assert!(a < 0.2);
}

#[test]
fn crossfader_input_is_clamped() {
    for c in [CrossfaderCurve::Linear, CrossfaderCurve::ConstantPower, CrossfaderCurve::Cut] {
        assert_eq!(crossfader_gains(-5.0, c), crossfader_gains(-1.0, c));
        assert_eq!(crossfader_gains(5.0, c), crossfader_gains(1.0, c));
    }
}

// --- Engine routing ---

fn engine_with_both_playing() -> Engine {
    let mut e = Engine::new();
    e.deck_mut(DeckId::A).load(dc_track(0.5, 50_000));
    e.deck_mut(DeckId::B).load(dc_track(0.25, 50_000));
    e.deck_mut(DeckId::A).play_pause();
    e.deck_mut(DeckId::B).play_pause();
    e.set_crossfader_curve(CrossfaderCurve::Linear);
    e
}

fn process(e: &mut Engine, frames: usize) -> (Vec<f32>, Vec<f32>) {
    let mut master = vec![9.0; frames * 2];
    let mut cue = vec![9.0; frames * 2];
    e.process(&mut master, &mut cue);
    (master, cue)
}

#[test]
fn crossfader_selects_deck_on_master() {
    let mut e = engine_with_both_playing();
    e.set_crossfader(-1.0);
    let (m, _) = process(&mut e, 32);
    assert!(m.iter().all(|&s| approx(s, 0.5)));
    e.set_crossfader(1.0);
    let (m, _) = process(&mut e, 32);
    assert!(m.iter().all(|&s| approx(s, 0.25)));
}

#[test]
fn centred_linear_crossfader_mixes_both() {
    let mut e = engine_with_both_playing();
    e.set_crossfader(0.0);
    let (m, _) = process(&mut e, 8);
    assert!(m.iter().all(|&s| approx(s, 0.375)));
}

#[test]
fn channel_fader_scales_its_deck() {
    let mut e = engine_with_both_playing();
    e.set_crossfader(-1.0);
    e.set_channel_fader(DeckId::A, 0.0);
    let (m, _) = process(&mut e, 8);
    assert!(m.iter().all(|&s| s == 0.0));
}

#[test]
fn headphone_cue_is_pre_fader() {
    let mut e = engine_with_both_playing();
    e.set_channel_fader(DeckId::B, 0.0);
    e.set_headphone_cue(DeckId::B, true);
    e.set_cue_mix(0.0);
    let (_, c) = process(&mut e, 8);
    assert!(c.iter().all(|&s| approx(s, 0.25)));
}

#[test]
fn cue_bus_is_silent_with_nothing_cued() {
    let mut e = engine_with_both_playing();
    e.set_cue_mix(0.0);
    let (_, c) = process(&mut e, 8);
    assert!(c.iter().all(|&s| s == 0.0));
}

#[test]
fn cue_mix_fully_right_sends_master_to_headphones() {
    let mut e = engine_with_both_playing();
    e.set_crossfader(-1.0);
    e.set_headphone_cue(DeckId::B, true);
    e.set_cue_mix(1.0);
    let (m, c) = process(&mut e, 8);
    assert_eq!(m, c);
}

#[test]
fn blocks_larger_than_the_internal_buffer_are_processed_fully() {
    let mut e = engine_with_both_playing();
    e.set_crossfader(-1.0);
    let (m, _) = process(&mut e, 20_000);
    assert!(m.iter().all(|&s| approx(s, 0.5)));
    assert_eq!(e.deck(DeckId::A).position(), 20_000.0);
}

#[test]
fn deck_id_other_flips() {
    assert_eq!(DeckId::A.other(), DeckId::B);
    assert_eq!(DeckId::B.other(), DeckId::A);
}
