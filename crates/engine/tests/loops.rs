//! Looping on a deck: the playhead wraps inside the loop until the loop is cleared.

use engine::{channel, Command, Deck, DeckId, Engine, Track};
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

/// Left channel only, which reads back as the frame index of each rendered sample.
fn left(out: &[f32]) -> Vec<f32> {
    out.iter().step_by(2).copied().collect()
}

fn playing_deck(frames: usize) -> Deck {
    let mut deck = Deck::new();
    deck.load(ramp_track(frames));
    deck.play_pause();
    deck
}

#[test]
fn a_loop_wraps_the_playhead_back_to_its_start() {
    let mut deck = playing_deck(1000);
    deck.set_loop(Some((2.0, 6.0)));
    assert_eq!(deck.loop_span(), Some((2.0, 6.0)));
    let out = render(&mut deck, 10);
    assert_eq!(
        left(&out),
        vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 2.0, 3.0, 4.0, 5.0]
    );
    assert_eq!(deck.position(), 2.0, "the tenth frame left it one wrap in");
}

#[test]
fn the_wrap_keeps_the_part_of_the_step_past_the_loop_end() {
    let mut deck = playing_deck(1000);
    deck.set_rate(1.5);
    deck.set_loop(Some((0.0, 5.0)));
    render(&mut deck, 4);
    // Steps land on 1.5, 3.0, 4.5 and then 6.0, which is 1.0 past the end.
    assert_eq!(deck.position(), 1.0);
}

#[test]
fn clearing_the_loop_lets_playback_run_past_the_end() {
    let mut deck = playing_deck(1000);
    deck.set_loop(Some((0.0, 4.0)));
    render(&mut deck, 4);
    assert_eq!(deck.position(), 0.0);
    deck.set_loop(None);
    assert_eq!(deck.loop_span(), None);
    let out = render(&mut deck, 6);
    assert_eq!(left(&out), vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn a_loop_outside_the_track_is_clamped_and_an_empty_one_is_refused() {
    let mut deck = playing_deck(100);
    deck.set_loop(Some((-10.0, 500.0)));
    assert_eq!(deck.loop_span(), Some((0.0, 100.0)));
    deck.set_loop(Some((50.0, 50.0)));
    assert_eq!(
        deck.loop_span(),
        None,
        "a zero length loop has nothing to play"
    );
    deck.set_loop(Some((60.0, 40.0)));
    assert_eq!(
        deck.loop_span(),
        None,
        "an end before its start is not a loop"
    );
}

#[test]
fn loading_a_new_track_clears_the_loop() {
    let mut deck = playing_deck(100);
    deck.set_loop(Some((10.0, 20.0)));
    deck.load(ramp_track(100));
    assert_eq!(deck.loop_span(), None, "the new track has its own loops");
}

#[test]
fn seeking_past_the_loop_end_leaves_the_loop_behind() {
    let mut deck = playing_deck(1000);
    deck.set_loop(Some((2.0, 6.0)));
    deck.seek(100.0);
    let out = render(&mut deck, 3);
    assert_eq!(
        left(&out),
        vec![100.0, 101.0, 102.0],
        "a jump out of the loop keeps playing from where it landed"
    );
}

#[test]
fn a_loop_set_while_paused_starts_wrapping_when_playback_resumes() {
    let mut deck = Deck::new();
    deck.load(ramp_track(1000));
    deck.set_loop(Some((0.0, 3.0)));
    deck.play_pause();
    let out = render(&mut deck, 5);
    assert_eq!(left(&out), vec![0.0, 1.0, 2.0, 0.0, 1.0]);
}

#[test]
fn the_snapshot_reports_the_loop_the_engine_is_playing() {
    let (mut handle, mut processor) = channel(Engine::new(), 16);
    handle
        .send(Command::Load(DeckId::A, ramp_track(1000)))
        .unwrap();
    handle
        .send(Command::SetLoop(DeckId::A, Some((4.0, 8.0))))
        .unwrap();
    let (mut m, mut c) = (vec![0.0; 32], vec![0.0; 32]);
    processor.process(&mut m, &mut c);
    assert_eq!(handle.snapshot().decks[0].loop_span, Some((4.0, 8.0)));

    handle.send(Command::SetLoop(DeckId::A, None)).unwrap();
    processor.process(&mut m, &mut c);
    assert_eq!(handle.snapshot().decks[0].loop_span, None);
}
