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

pub use loader::{peak_envelope, waveform_envelope};
