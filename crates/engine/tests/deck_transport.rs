//! Transport and CDJ-style cue behaviour for a single deck.

use engine::{Deck, Track};
use std::sync::Arc;

const RATE: u32 = 48_000;

/// A stereo track whose left sample at frame n equals n, so position is readable from output.
fn ramp_track(frames: usize) -> Arc<Track> {
    let mut data = Vec::with_capacity(frames * 2);
    for n in 0..frames {
        data.push(n as f32);
        data.push(-(n as f32));
    }
    Arc::new(Track::from_interleaved(data, RATE))
}

fn render(deck: &mut Deck, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * 2];
    deck.render(&mut out);
    out
}

#[test]
fn empty_deck_renders_silence_and_ignores_play() {
    let mut deck = Deck::new();
    deck.play_pause();
    assert!(!deck.is_playing());
    assert!(render(&mut deck, 64).iter().all(|&s| s == 0.0));
}

#[test]
fn loaded_deck_is_paused_at_start() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    assert!(!deck.is_playing());
    assert_eq!(deck.position(), 0.0);
    assert!(render(&mut deck, 16).iter().all(|&s| s == 0.0));
}

#[test]
fn playing_reads_the_track_and_advances() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.play_pause();
    let out = render(&mut deck, 4);
    assert_eq!(out, vec![0.0, -0.0, 1.0, -1.0, 2.0, -2.0, 3.0, -3.0]);
    assert_eq!(deck.position(), 4.0);
}

#[test]
fn play_pause_toggles() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.play_pause();
    render(&mut deck, 10);
    deck.play_pause();
    assert!(!deck.is_playing());
    render(&mut deck, 10);
    assert_eq!(deck.position(), 10.0);
}

#[test]
fn rate_changes_playback_speed_with_interpolation() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.set_rate(0.5);
    deck.play_pause();
    let out = render(&mut deck, 3);
    assert_eq!(out[0], 0.0);
    assert!((out[2] - 0.5).abs() < 1e-6);
    assert!((out[4] - 1.0).abs() < 1e-6);
    assert_eq!(deck.position(), 1.5);
}

#[test]
fn playback_stops_at_end_of_track_and_pads_with_silence() {
    let mut deck = Deck::new();
    deck.load(ramp_track(3));
    deck.play_pause();
    let out = render(&mut deck, 5);
    assert_eq!(&out[..6], &[0.0, -0.0, 1.0, -1.0, 2.0, -2.0]);
    assert_eq!(&out[6..], &[0.0; 4]);
    assert!(!deck.is_playing());
}

// --- Main cue, Pioneer style ---

#[test]
fn cue_while_paused_sets_cue_point_at_position() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.seek(200.0);
    deck.cue_press();
    deck.cue_release();
    assert_eq!(deck.cue_point(), 200.0);
    assert_eq!(deck.position(), 200.0);
    assert!(!deck.is_playing());
}

#[test]
fn holding_cue_while_paused_previews_then_snaps_back() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.seek(100.0);
    deck.cue_press();
    assert!(deck.is_playing());
    render(&mut deck, 50);
    assert_eq!(deck.position(), 150.0);
    deck.cue_release();
    assert!(!deck.is_playing());
    assert_eq!(deck.position(), 100.0);
}

#[test]
fn pressing_play_during_cue_preview_keeps_playing_after_release() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.cue_press();
    render(&mut deck, 20);
    deck.play_pause();
    deck.cue_release();
    assert!(deck.is_playing());
    assert_eq!(deck.position(), 20.0);
}

#[test]
fn cue_while_playing_returns_to_cue_point_and_pauses() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.seek(50.0);
    deck.cue_press();
    deck.cue_release();
    deck.play_pause();
    render(&mut deck, 100);
    deck.cue_press();
    assert!(!deck.is_playing());
    assert_eq!(deck.position(), 50.0);
    deck.cue_release();
    assert_eq!(deck.position(), 50.0);
    assert!(!deck.is_playing());
}

// --- Hot cues ---

#[test]
fn hot_cue_sets_when_empty_and_jumps_when_set() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.seek(300.0);
    deck.hot_cue(2);
    assert_eq!(deck.hot_cue_position(2), Some(300.0));
    deck.seek(700.0);
    deck.hot_cue(2);
    assert_eq!(deck.position(), 300.0);
}

#[test]
fn hot_cue_jump_keeps_play_state() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.hot_cue(0);
    deck.play_pause();
    render(&mut deck, 100);
    deck.hot_cue(0);
    assert!(deck.is_playing());
    assert_eq!(deck.position(), 0.0);
}

#[test]
fn clear_hot_cue_empties_the_slot() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.hot_cue(7);
    deck.clear_hot_cue(7);
    assert_eq!(deck.hot_cue_position(7), None);
}

#[test]
fn out_of_range_hot_cue_index_is_ignored() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.hot_cue(8);
    deck.clear_hot_cue(99);
    assert_eq!(deck.position(), 0.0);
}

#[test]
fn loading_a_new_track_resets_cues_and_stops() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.seek(100.0);
    deck.hot_cue(0);
    deck.cue_press();
    deck.cue_release();
    deck.play_pause();
    deck.load(ramp_track(500));
    assert!(!deck.is_playing());
    assert_eq!(deck.position(), 0.0);
    assert_eq!(deck.cue_point(), 0.0);
    assert_eq!(deck.hot_cue_position(0), None);
}

#[test]
fn seek_is_clamped_to_track_length() {
    let mut deck = Deck::new();
    deck.load(ramp_track(100));
    deck.seek(1e9);
    assert_eq!(deck.position(), 100.0);
    deck.seek(-5.0);
    assert_eq!(deck.position(), 0.0);
}
