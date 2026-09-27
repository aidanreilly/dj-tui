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

/// A steady sine, the signal a comb filter shows up in most clearly.
fn sine(freq: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .flat_map(|i| {
            let s = 0.5 * (std::f32::consts::TAU * freq * i as f32 / FS).sin();
            [s, s]
        })
        .collect()
}

#[test]
fn the_flanger_colours_a_steady_tone() {
    let mut fx = slot(FxKind::Flanger, 1.0);
    settle(&mut fx);
    let dry = sine(440.0, 4 * BEAT as usize);
    let mut wet = dry.clone();
    fx.process(&mut wet);
    let diff = dry
        .iter()
        .zip(&wet)
        .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(diff > 0.05, "the comb is audible: {diff}");
}

#[test]
fn the_flanger_sweep_repeats_once_every_lfo_period() {
    use engine::fx::FLANGER_BEATS;
    let mut fx = slot(FxKind::Flanger, 1.0);
    settle(&mut fx);
    let period = (FLANGER_BEATS * BEAT as f64) as usize;
    let mut buf = sine(440.0, 3 * period);
    fx.process(&mut buf);
    let out = left(&buf);

    // One period on, the sweep is back where it was, so the tone is coloured the same way.
    let window = |from: usize| -> Vec<f32> { out[from..from + 2_000].to_vec() };
    let dist = |a: &[f32], b: &[f32]| -> f32 {
        a.iter()
            .zip(b)
            .fold(0.0f32, |m, (x, y)| m.max((x - y).abs()))
    };
    let base = window(period);
    assert!(
        dist(&base, &window(2 * period)) < 0.02,
        "a period later it lines up: {}",
        dist(&base, &window(2 * period))
    );
    assert!(
        dist(&base, &window(period + period / 2)) > 0.02,
        "half a period later it does not"
    );
}

#[test]
fn the_flanger_stops_when_it_is_switched_off() {
    let mut fx = slot(FxKind::Flanger, 1.0);
    settle(&mut fx);
    let mut warm = sine(440.0, BEAT as usize);
    fx.process(&mut warm);
    fx.set_on(false);
    assert!(!fx.is_ringing(), "a flanger has nothing to ring out");
    settle(&mut fx);
    let dry = sine(440.0, 2_000);
    let mut buf = dry.clone();
    fx.process(&mut buf);
    assert_eq!(buf, dry);
}

/// Frames until the tail falls below an audible level, feeding silence.
fn tail_frames(fx: &mut FxSlot) -> usize {
    let mut frames = 0;
    for _ in 0..200 {
        let mut block = silence(4_800);
        fx.process(&mut block);
        let peak = left(&block).iter().fold(0.0f32, |m, s| m.max(s.abs()));
        if peak < 1e-3 {
            break;
        }
        frames += 4_800;
    }
    frames
}

#[test]
fn the_reverb_keeps_sounding_after_the_input_stops() {
    let mut fx = slot(FxKind::Reverb, 1.0);
    settle(&mut fx);
    let mut buf = impulse(4_800);
    fx.process(&mut buf);
    assert!(tail_frames(&mut fx) > 4_800, "a tail worth the name");
}

#[test]
fn a_bigger_room_rings_for_longer() {
    let measure = |size: f32| {
        let mut fx = slot(FxKind::Reverb, 1.0);
        fx.set_reverb_size(size);
        settle(&mut fx);
        let mut buf = impulse(4_800);
        fx.process(&mut buf);
        tail_frames(&mut fx)
    };
    assert!(
        measure(0.9) > measure(0.2),
        "size lengthens the tail: {} vs {}",
        measure(0.9),
        measure(0.2)
    );
}

#[test]
fn damping_takes_the_top_off_the_tail() {
    let measure = |damping: f32| {
        let mut fx = slot(FxKind::Reverb, 1.0);
        fx.set_reverb_damping(damping);
        settle(&mut fx);
        let mut buf = sine(4_000.0, 4_800);
        fx.process(&mut buf);
        let mut tail = silence(9_600);
        fx.process(&mut tail);
        left(&tail).iter().fold(0.0f32, |m, s| m.max(s.abs()))
    };
    assert!(
        measure(0.9) < measure(0.1),
        "a damped room loses its highs sooner: {} vs {}",
        measure(0.9),
        measure(0.1)
    );
}

#[test]
fn the_reverb_rings_out_after_it_is_switched_off() {
    let mut fx = slot(FxKind::Reverb, 1.0);
    settle(&mut fx);
    let mut buf = impulse(4_800);
    fx.process(&mut buf);
    fx.set_on(false);
    assert!(fx.is_ringing());
    // The slot stops reporting a ring once the room is quieter than the tail test's floor,
    // which takes a little longer than the tail stays audible.
    let mut blocks = 0;
    while fx.is_ringing() && blocks < 400 {
        let mut block = silence(4_800);
        fx.process(&mut block);
        blocks += 1;
    }
    assert!(!fx.is_ringing(), "quiet in the end after {blocks} blocks");
}

#[test]
fn the_two_knobs_start_where_the_defaults_are() {
    let fx = FxSlot::new(FS);
    assert_eq!(fx.param(0), 0.5);
    assert_eq!(fx.param(1), 0.5);
    assert_eq!(fx.param(9), 0.0, "there are only two");
}

#[test]
fn the_first_knob_sets_the_echo_time_in_beats() {
    let time = |p: f32| {
        let mut fx = slot(FxKind::Echo, 1.0);
        fx.set_param(0, p);
        settle(&mut fx);
        let mut buf = impulse(4 * BEAT as usize);
        fx.process(&mut buf);
        let out = left(&buf);
        // First repeat after the dry hit.
        out.iter()
            .enumerate()
            .skip(100)
            .find(|(_, v)| v.abs() > 0.5)
            .map(|(i, _)| i as f64 / BEAT as f64)
            .expect("a repeat")
    };
    assert!((time(0.5) - ECHO_BEATS).abs() < 0.01, "{}", time(0.5));
    assert!(time(0.0) < time(0.5), "left is shorter: {}", time(0.0));
    assert!(time(1.0) > time(0.5), "right is longer: {}", time(1.0));
    assert!((time(0.0) - 0.125).abs() < 0.01, "down to an eighth beat");
    assert!((time(1.0) - 2.0).abs() < 0.02, "up to two beats");
}

#[test]
fn the_second_knob_sets_how_long_the_echo_keeps_repeating() {
    // Everything the repeats add up to over twenty beats, which grows with the feedback.
    let energy = |p: f32| {
        let mut fx = slot(FxKind::Echo, 1.0);
        fx.set_param(1, p);
        settle(&mut fx);
        let mut buf = impulse(BEAT as usize / 4);
        fx.process(&mut buf);
        let mut tail = silence(20 * BEAT as usize);
        fx.process(&mut tail);
        left(&tail).iter().map(|s| s.abs()).sum::<f32>()
    };
    let (few, many) = (energy(0.0), energy(1.0));
    assert!(
        many > few * 2.0,
        "more feedback rings longer: {few} vs {many}"
    );
}

#[test]
fn the_knobs_drive_the_reverb_room() {
    let tail = |size: f32| {
        let mut fx = slot(FxKind::Reverb, 1.0);
        fx.set_param(0, size);
        settle(&mut fx);
        let mut buf = impulse(4_800);
        fx.process(&mut buf);
        tail_frames(&mut fx)
    };
    assert!(tail(1.0) > tail(0.0), "{} vs {}", tail(1.0), tail(0.0));
}

#[test]
fn the_knobs_drive_the_crusher_and_the_flanger() {
    let steps = |p: f32| {
        let mut fx = slot(FxKind::Bitcrusher, 1.0);
        fx.set_param(0, p);
        settle(&mut fx);
        let mut buf: Vec<f32> = (0..4_000)
            .flat_map(|i| {
                let s = (i as f32 / 4_000.0) * 2.0 - 1.0;
                [s, s]
            })
            .collect();
        fx.process(&mut buf);
        left(&buf)
            .iter()
            .map(|s| s.to_bits())
            .collect::<std::collections::HashSet<u32>>()
            .len()
    };
    assert!(steps(1.0) < steps(0.0), "{} vs {}", steps(1.0), steps(0.0));

    let sweep = |p: f32| {
        let mut fx = slot(FxKind::Flanger, 1.0);
        fx.set_param(1, p);
        settle(&mut fx);
        let dry = sine(440.0, 2 * BEAT as usize);
        let mut wet = dry.clone();
        fx.process(&mut wet);
        dry.iter()
            .zip(&wet)
            .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()))
    };
    assert!(sweep(1.0) > sweep(0.1), "{} vs {}", sweep(1.0), sweep(0.1));
}
