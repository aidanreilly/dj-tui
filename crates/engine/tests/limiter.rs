//! The master bus limiter: nothing leaves the mixer above full scale.

use engine::dsp::{Limiter, LIMIT_CEILING};
use engine::{channel, Command, DeckId::*, Engine, Track};
use std::sync::Arc;

const FS: f32 = 48_000.0;

fn sine(freq: f32, amp: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .flat_map(|i| {
            let s = amp * (std::f32::consts::TAU * freq * i as f32 / FS).sin();
            [s, s]
        })
        .collect()
}

fn peak(buf: &[f32]) -> f32 {
    buf.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

#[test]
fn a_signal_under_the_ceiling_comes_through_untouched() {
    let mut limiter = Limiter::new(FS);
    let mut buf = sine(440.0, 0.5, 4_800);
    let before = buf.clone();
    limiter.process(&mut buf);
    assert_eq!(buf, before);
    assert_eq!(limiter.reduction_db(), 0.0);
}

#[test]
fn nothing_gets_out_above_the_ceiling() {
    let mut limiter = Limiter::new(FS);
    let mut buf = sine(220.0, 4.0, 48_000);
    limiter.process(&mut buf);
    assert!(
        peak(&buf) <= LIMIT_CEILING + 1e-6,
        "four times full scale comes out at {}",
        peak(&buf)
    );
    assert!(
        limiter.reduction_db() > 6.0,
        "and it says how hard it worked"
    );
}

#[test]
fn a_single_sample_spike_is_caught_on_the_spot() {
    let mut limiter = Limiter::new(FS);
    let mut buf = vec![0.0; 200];
    buf[100] = 8.0;
    buf[101] = 8.0;
    limiter.process(&mut buf);
    assert!(peak(&buf) <= LIMIT_CEILING + 1e-6, "{}", peak(&buf));
}

#[test]
fn both_channels_move_together_so_the_image_holds_still() {
    let mut limiter = Limiter::new(FS);
    // Left is loud, right is quiet; the ratio between them must survive.
    let mut buf: Vec<f32> = (0..4_800)
        .flat_map(|i| {
            let s = (std::f32::consts::TAU * 440.0 * i as f32 / FS).sin();
            [2.0 * s, 0.5 * s]
        })
        .collect();
    limiter.process(&mut buf);
    for frame in buf.as_chunks::<2>().0 {
        if frame[0].abs() > 0.05 {
            let ratio = frame[1] / frame[0];
            assert!((ratio - 0.25).abs() < 1e-3, "{ratio}");
        }
    }
}

#[test]
fn the_level_comes_back_after_a_loud_passage() {
    let mut limiter = Limiter::new(FS);
    let mut loud = sine(440.0, 4.0, 4_800);
    limiter.process(&mut loud);
    assert!(
        limiter.reduction_db() > 6.0,
        "working hard on the loud part"
    );
    // A couple of seconds of quiet is long enough for the gain to return.
    let mut gap = sine(440.0, 0.1, 96_000);
    limiter.process(&mut gap);
    // The gap starts with the gain still down, so read that off before asking about what
    // comes next.
    limiter.reduction_db();
    let mut quiet = sine(440.0, 0.5, 4_800);
    let before = quiet.clone();
    limiter.process(&mut quiet);
    assert!(
        (peak(&quiet) - peak(&before)).abs() < 1e-3,
        "{} against {}",
        peak(&quiet),
        peak(&before)
    );
    assert_eq!(limiter.reduction_db(), 0.0, "and it reports nothing to do");
}

#[test]
fn the_master_bus_is_limited_and_the_cue_bus_hears_the_same_thing() {
    let (mut h, mut p) = channel(Engine::with_sample_rate(48_000), 16);
    let loud: Vec<f32> = sine(220.0, 0.9, 48_000);
    let track = Arc::new(Track::from_interleaved(loud, 48_000));
    for deck in [A, B] {
        h.send(Command::Load(deck, track.clone())).unwrap();
        h.send(Command::PlayPause(deck)).unwrap();
        h.send(Command::SetTrim(deck, 12.0)).unwrap();
        h.send(Command::SetHeadphoneCue(deck, true)).unwrap();
    }
    // Both decks up the middle, so the master sums well past full scale.
    h.send(Command::SetCrossfader(0.0)).unwrap();
    h.send(Command::SetCueMix(1.0)).unwrap();
    let (mut m, mut c) = (vec![0.0; 4_096], vec![0.0; 4_096]);
    p.process(&mut m, &mut c);
    p.process(&mut m, &mut c);
    assert!(peak(&m) <= LIMIT_CEILING + 1e-6, "master at {}", peak(&m));
    assert!(
        peak(&c) <= LIMIT_CEILING + 1e-6,
        "the cue bus takes the limited master, not the raw one: {}",
        peak(&c)
    );
}

#[test]
fn the_meters_show_what_left_the_mixer() {
    let (mut h, mut p) = channel(Engine::with_sample_rate(48_000), 16);
    let track = Arc::new(Track::from_interleaved(sine(220.0, 0.9, 48_000), 48_000));
    h.send(Command::Load(A, track)).unwrap();
    h.send(Command::PlayPause(A)).unwrap();
    h.send(Command::SetTrim(A, 12.0)).unwrap();
    h.send(Command::SetCrossfader(-1.0)).unwrap();
    let (mut m, mut c) = (vec![0.0; 4_096], vec![0.0; 4_096]);
    p.process(&mut m, &mut c);
    p.process(&mut m, &mut c);
    let meters = h.take_meters();
    assert!(
        meters.master <= LIMIT_CEILING + 1e-6,
        "the master meter reads the limited bus: {}",
        meters.master
    );
    assert!(meters.channels[0] > 1.0, "the channel meter is pre-limiter");
}
