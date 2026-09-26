//! Synthetic material for `--demo`, until decoding lands in M1.

use engine::Track;
use std::f64::consts::TAU;

const CLICK_DECAY_SECS: f64 = 0.01;
const CLICK_GATE_SECS: f64 = 0.05;

/// A metronome track: a short decaying tone on every beat, accented on each downbeat.
pub fn click_track(bpm: f64, secs: f64, sample_rate: u32) -> Track {
    let rate = sample_rate as f64;
    let frames = (secs * rate) as usize;
    let beat = rate * 60.0 / bpm;
    let tone_hz = (rate / 8.0).min(1000.0);
    let mut data = Vec::with_capacity(frames * 2);
    for n in 0..frames {
        let beat_index = (n as f64 / beat).floor();
        let t = (n as f64 - beat_index * beat) / rate;
        let accent = if beat_index as u64 % 4 == 0 { 1.0 } else { 0.7 };
        let s = if t < CLICK_GATE_SECS {
            accent * (-t / CLICK_DECAY_SECS).exp() * (TAU * tone_hz * t).cos()
        } else {
            0.0
        } as f32;
        data.push(s);
        data.push(s);
    }
    Track::from_interleaved(data, sample_rate)
}

/// Whole-track peak envelope with `points` values, normalised so the loudest is 1.
pub fn peak_envelope(track: &Track, points: usize) -> Vec<f32> {
    let frames = track.frames();
    if frames == 0 || points == 0 {
        return vec![0.0; points];
    }
    let mut env: Vec<f32> = (0..points)
        .map(|j| {
            let start = j * frames / points;
            let end = ((j + 1) * frames / points).max(start + 1).min(frames);
            (start..end)
                .map(|i| {
                    let (l, r) = track.frame_at(i as f64);
                    l.abs().max(r.abs())
                })
                .fold(0.0, f32::max)
        })
        .collect();
    let max = env.iter().copied().fold(0.0, f32::max);
    if max > 0.0 {
        env.iter_mut().for_each(|v| *v /= max);
    }
    env
}
