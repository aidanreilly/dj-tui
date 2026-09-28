#![allow(dead_code)] // shared by several test binaries; each uses only part
use engine::Track;
use std::f32::consts::TAU;

pub const FS: u32 = 48_000;

fn track(mono: Vec<f32>) -> Track {
    Track::from_interleaved(mono.iter().flat_map(|&s| [s, s]).collect(), FS)
}

/// Short decaying clicks every beat, starting at `first_beat` seconds.
pub fn clicks(bpm: f32, secs: f32, first_beat: f32) -> Track {
    let n = (secs * FS as f32) as usize;
    let beat = 60.0 / bpm;
    let mono = (0..n)
        .map(|i| {
            let t = i as f32 / FS as f32 - first_beat;
            if t < 0.0 {
                return 0.0;
            }
            let tb = t % beat;
            if tb < 0.03 {
                (-tb / 0.005).exp() * (TAU * 1500.0 * tb).sin()
            } else {
                0.0
            }
        })
        .collect();
    track(mono)
}

/// Kick on every beat, open hat on every off-beat, a held bass note: a house loop.
pub fn house(bpm: f32, secs: f32) -> Track {
    let n = (secs * FS as f32) as usize;
    let beat = 60.0 / bpm;
    let mut rng = 0x9e37_79b9u32;
    let mut prev = 0.0;
    let mono = (0..n)
        .map(|i| {
            let t = i as f32 / FS as f32;
            let tb = t % beat;
            let kick = (-tb * 20.0).exp() * (TAU * (48.0 + 80.0 * (-tb * 35.0).exp()) * tb).sin();
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let noise = rng as f32 / u32::MAX as f32 * 2.0 - 1.0;
            let hp = noise - prev;
            prev = noise;
            let off = (tb - beat / 2.0).abs();
            let hat = if off < 0.04 {
                0.25 * hp * (1.0 - off / 0.04)
            } else {
                0.0
            };
            let bass = 0.2 * (TAU * 55.0 * t).sin();
            0.7 * kick + hat + bass
        })
        .collect();
    track(mono)
}

/// A sixteenth-note line with a kick on the beat and accents on a five-against-four
/// cross rhythm, which is what an acid pattern does to the onset envelope.
pub fn cross_rhythm(bpm: f32, secs: f32) -> Track {
    let n = (secs * FS as f32) as usize;
    let step = 60.0 / bpm / 4.0;
    // Five accents to the bar against four kicks, quantised to the sixteenth grid.
    let accents = [0, 3, 6, 10, 13];
    // The line is quiet, so the accents own the envelope the way a 303 does.
    const LINE: f32 = 0.15;
    const ACCENT: f32 = 0.4;
    let mono = (0..n)
        .map(|i| {
            let t = i as f32 / FS as f32;
            let k = (t / step) as usize;
            let ts = t - k as f32 * step;
            let tick = if ts < 0.02 {
                (-ts / 0.004).exp() * (TAU * 2200.0 * ts).sin()
            } else {
                0.0
            };
            let accent = if accents.contains(&(k % 16)) {
                ACCENT
            } else {
                0.0
            };
            let kick = if k.is_multiple_of(4) {
                0.3 * (-ts * 20.0).exp() * (TAU * (48.0 + 80.0 * (-ts * 35.0).exp()) * ts).sin()
            } else {
                0.0
            };
            0.5 * ((LINE + accent) * tick + kick)
        })
        .collect();
    track(mono)
}

/// Sustained notes (Hz) with a few harmonics, as a crude pad.
pub fn chord(freqs: &[f32], secs: f32) -> Track {
    let n = (secs * FS as f32) as usize;
    let mono = (0..n)
        .map(|i| {
            let t = i as f32 / FS as f32;
            freqs
                .iter()
                .map(|f| {
                    (1..=4)
                        .map(|h| (TAU * f * h as f32 * t).sin() / h as f32)
                        .sum::<f32>()
                })
                .sum::<f32>()
                * 0.1
        })
        .collect();
    track(mono)
}

pub fn silence(secs: f32) -> Track {
    track(vec![0.0; (secs * FS as f32) as usize])
}

/// Equal-tempered frequency of MIDI note `n`.
pub fn midi(n: i32) -> f32 {
    440.0 * 2f32.powf((n - 69) as f32 / 12.0)
}
