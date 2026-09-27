//! Renders the three waveform modes for a synthetic dance track, for eyeballing colours.
//!     cargo run --release --example waveform_modes -- /tmp/out
//! Writes one PPM per mode, composited on a dark background.

use engine::Track;
use loader::{band_envelope, waveform_envelope, ENVELOPE_POINTS};
use std::f32::consts::TAU;
use std::io::Write;
use tui::pixel::{Palette, Wave, WaveformBitmaps, WaveformMode};

const FS: u32 = 48_000;

/// 64 bars at 128 BPM: drums intro, full drop, mid-only breakdown, drop, hats outro.
fn synth() -> Track {
    let beat = 60.0 / 128.0;
    let secs = beat * 4.0 * 64.0;
    let n = (secs * FS as f32) as usize;
    let mut rng = 0x1234_5678u32;
    let mut noise = move || {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        rng as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let mut hp = 0.0f32;
    let mut prev = 0.0f32;
    let mut data = Vec::with_capacity(n * 2);
    for i in 0..n {
        let t = i as f32 / FS as f32;
        let bar = (t / (beat * 4.0)) as usize;
        let tb = t % beat;
        let (kick_on, bass_on, vox_on, hat_on) = match bar {
            0..=7 => (true, false, false, true),
            8..=23 => (true, true, true, true),
            24..=35 => (false, false, true, false),
            36..=55 => (true, true, true, true),
            _ => (false, false, false, true),
        };
        let kick = if kick_on {
            (-tb * 18.0).exp() * (TAU * (50.0 + 90.0 * (-tb * 30.0).exp()) * tb).sin()
        } else {
            0.0
        };
        let bass = if bass_on && tb > beat * 0.5 {
            0.45 * (TAU * 55.0 * t).sin()
        } else {
            0.0
        };
        let vox = if vox_on {
            0.3 * (TAU * 700.0 * t + 3.0 * (TAU * 5.0 * t).sin()).sin()
                * (0.6 + 0.4 * (TAU * 0.25 * t).sin())
        } else {
            0.0
        };
        let x = noise();
        hp = 0.2 * (hp + x - prev);
        prev = x;
        let hat = if hat_on && (tb - beat * 0.5).abs() < 0.03 {
            2.5 * hp
        } else {
            0.0
        };
        let s = (0.9 * kick + bass + vox + hat).clamp(-1.0, 1.0);
        data.extend_from_slice(&[s, s]);
    }
    Track::from_interleaved(data, FS)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let track = synth();
    let ranges = waveform_envelope(&track, ENVELOPE_POINTS);
    let bands = band_envelope(&track, ENVELOPE_POINTS);
    let (w, h) = (1360, 160);
    for (mode, name) in [
        (WaveformMode::Blue, "blue"),
        (WaveformMode::Rgb, "rgb"),
        (WaveformMode::ThreeBand, "3band"),
    ] {
        let wave = Wave {
            ranges: &ranges,
            bands: &bands,
            mode,
        };
        let img = WaveformBitmaps::rasterize(&wave, w, h, &Palette::default())
            .compose(Some(w / 3), &Palette::default());
        let mut f = std::fs::File::create(format!("{out}/{name}.ppm")).unwrap();
        writeln!(f, "P6 {w} {h} 255").unwrap();
        for p in img.pixels() {
            let a = p[3] as f32 / 255.0;
            let bg = 17.0;
            f.write_all(&[0, 1, 2].map(|c| (p[c] as f32 * a + bg * (1.0 - a)).round() as u8))
                .unwrap();
        }
    }
}
