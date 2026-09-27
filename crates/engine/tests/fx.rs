//! The per-deck effect slot: one unit at a time, tempo aware, with tails on switch-off.

use engine::fx::{FxKind, FxSlot, ECHO_BEATS, ECHO_FEEDBACK};

const FS: f32 = 48_000.0;
const BEAT: f32 = 24_000.0;

fn slot(kind: FxKind, wet: f32) -> FxSlot {
    let mut fx = FxSlot::new(FS);
    fx.set_beat_frames(BEAT);
    fx.set_kind(kind);
    fx.set_wet(wet);
    fx.set_on(true);
    fx
}

fn silence(frames: usize) -> Vec<f32> {
    vec![0.0; frames * 2]
}

/// One full-scale frame, then silence.
fn impulse(frames: usize) -> Vec<f32> {
    let mut buf = silence(frames);
    buf[0] = 1.0;
    buf[1] = 1.0;
    buf
}

/// Left channel of each frame.
fn left(buf: &[f32]) -> Vec<f32> {
    buf.iter().step_by(2).copied().collect()
}

/// Run enough silence through the slot for its gain smoothing to settle.
fn settle(fx: &mut FxSlot) {
    let mut warm = silence(4_800);
    fx.process(&mut warm);
}

#[test]
fn an_effect_switched_off_passes_the_signal_through_untouched() {
    let mut fx = FxSlot::new(FS);
    fx.set_beat_frames(BEAT);
    fx.set_wet(1.0);
    let mut buf = impulse(1_000);
    let before = buf.clone();
    fx.process(&mut buf);
    assert_eq!(buf, before);
    assert!(!fx.is_ringing());
}

#[test]
fn a_dry_slot_leaves_the_signal_alone_even_switched_on() {
    let mut fx = slot(FxKind::Echo, 0.0);
    settle(&mut fx);
    let mut buf = impulse(1_000);
    let before = buf.clone();
    fx.process(&mut buf);
    assert_eq!(buf, before, "wet at zero is a bypass");
}

#[test]
fn the_echo_repeats_at_the_beat_fraction_and_decays_by_the_feedback() {
    let mut fx = slot(FxKind::Echo, 1.0);
    settle(&mut fx);
    let mut buf = impulse(3 * BEAT as usize);
    fx.process(&mut buf);
    let out = left(&buf);
    let delay = (ECHO_BEATS * BEAT as f64) as usize;

    assert_eq!(out[0], 1.0, "the dry hit passes through");
    assert!(
        (out[delay] - 1.0).abs() < 1e-3,
        "first repeat one echo time later: {}",
        out[delay]
    );
    assert!(
        (out[2 * delay] - ECHO_FEEDBACK as f32).abs() < 1e-3,
        "second repeat down by the feedback: {}",
        out[2 * delay]
    );
    assert!(
        out[delay / 2].abs() < 1e-6,
        "nothing between the repeats: {}",
        out[delay / 2]
    );
}

#[test]
fn the_echo_time_follows_the_tempo_it_is_given() {
    let mut fx = slot(FxKind::Echo, 1.0);
    fx.set_beat_frames(BEAT / 2.0);
    settle(&mut fx);
    let mut buf = impulse(2 * BEAT as usize);
    fx.process(&mut buf);
    let out = left(&buf);
    let delay = (ECHO_BEATS * BEAT as f64 / 2.0) as usize;
    assert!((out[delay] - 1.0).abs() < 1e-3, "{}", out[delay]);
}

#[test]
fn the_echo_rings_out_after_it_is_switched_off() {
    let mut fx = slot(FxKind::Echo, 1.0);
    settle(&mut fx);
    let mut buf = impulse(BEAT as usize / 4);
    fx.process(&mut buf);
    fx.set_on(false);
    assert!(fx.is_ringing(), "the delay line still holds the hit");

    let mut tail = silence(2 * BEAT as usize);
    fx.process(&mut tail);
    let peak = left(&tail).iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(peak > 0.3, "the repeats keep coming: {peak}");

    // Long enough for the feedback to fall away.
    for _ in 0..40 {
        let mut more = silence(BEAT as usize);
        fx.process(&mut more);
    }
    assert!(!fx.is_ringing(), "silence in the end");
}

#[test]
fn switching_off_stops_new_sound_entering_the_echo() {
    let mut fx = slot(FxKind::Echo, 1.0);
    settle(&mut fx);
    fx.set_on(false);
    // The input gate closes over a few milliseconds rather than snapping, so give it that.
    settle(&mut fx);
    let mut buf = impulse(2 * BEAT as usize);
    fx.process(&mut buf);
    let out = left(&buf);
    let delay = (ECHO_BEATS * BEAT as f64) as usize;
    assert_eq!(out[0], 1.0, "the dry signal still passes");
    assert!(
        out[delay].abs() < 1e-3,
        "what plays after the switch is not echoed: {}",
        out[delay]
    );
}

#[test]
fn the_bitcrusher_quantises_the_signal_and_stops_dead() {
    let mut fx = slot(FxKind::Bitcrusher, 1.0);
    settle(&mut fx);
    let mut buf: Vec<f32> = (0..4_000)
        .flat_map(|i| {
            let s = (i as f32 / 4_000.0) * 2.0 - 1.0;
            [s, s]
        })
        .collect();
    fx.process(&mut buf);
    let steps: std::collections::HashSet<u32> = left(&buf).iter().map(|s| s.to_bits()).collect();
    assert!(
        steps.len() < 300,
        "a ramp comes out in steps, not 4000 values: {}",
        steps.len()
    );

    fx.set_on(false);
    assert!(!fx.is_ringing(), "a crusher has no tail");
    settle(&mut fx);
    let mut ramp: Vec<f32> = (0..1_000).flat_map(|i| [i as f32 / 1_000.0; 2]).collect();
    let before = ramp.clone();
    fx.process(&mut ramp);
    assert_eq!(ramp, before, "off means clean again");
}

#[test]
fn cycling_passes_through_every_effect_and_comes_back() {
    let mut kind = FxKind::default();
    let mut seen = vec![kind];
    for _ in 0..8 {
        kind = kind.next();
        if kind == seen[0] {
            break;
        }
        seen.push(kind);
    }
    assert_eq!(kind, seen[0], "the cycle closes");
    assert!(seen.len() > 1, "more than one effect to choose from");
    assert!(seen.iter().all(|k| !k.name().is_empty()));
}

#[test]
fn the_slot_reports_what_it_is_set_to() {
    let mut fx = FxSlot::new(FS);
    assert!(!fx.is_on());
    fx.set_kind(FxKind::Bitcrusher);
    fx.set_on(true);
    fx.set_wet(0.6);
    assert_eq!(fx.kind(), FxKind::Bitcrusher);
    assert!(fx.is_on());
    assert!((fx.wet() - 0.6).abs() < 1e-6);
    fx.set_wet(2.0);
    assert_eq!(fx.wet(), 1.0, "wet is a fraction");
}
