//! Per-band overview envelopes for the coloured waveform modes (3-Band, RGB, Blue).

use engine::Track;
use loader::{band_envelope, waveform_envelope};

const FS: u32 = 48_000;

fn sine(freq: f32, amp: f32, secs: f32) -> Track {
    let data = (0..(FS as f32 * secs) as usize)
        .flat_map(|i| {
            let s = amp * (std::f32::consts::TAU * freq * i as f32 / FS as f32).sin();
            [s, s]
        })
        .collect();
    Track::from_interleaved(data, FS)
}

/// Mean of each band over the steady middle of the track.
fn middle(bands: &[[f32; 3]]) -> [f32; 3] {
    let mid = &bands[bands.len() / 4..bands.len() * 3 / 4];
    let mut sum = [0.0; 3];
    for b in mid {
        for i in 0..3 {
            sum[i] += b[i] / mid.len() as f32;
        }
    }
    sum
}

#[test]
fn has_one_entry_per_point() {
    assert_eq!(band_envelope(&sine(100.0, 0.5, 1.0), 64).len(), 64);
    assert_eq!(
        band_envelope(&Track::from_interleaved(vec![], FS), 16),
        vec![[0.0; 3]; 16]
    );
}

#[test]
fn bass_lands_in_the_low_band() {
    let [l, m, h] = middle(&band_envelope(&sine(50.0, 0.8, 2.0), 64));
    assert!((l - 1.0).abs() < 0.1, "low {l}");
    assert!(m < 0.1 && h < 0.05, "mid {m} high {h}");
}

#[test]
fn midrange_lands_in_the_mid_band() {
    let [l, m, h] = middle(&band_envelope(&sine(900.0, 0.8, 2.0), 64));
    assert!(m > 0.8, "mid {m}");
    assert!(l < 0.1 && h < 0.2, "low {l} high {h}");
}

#[test]
fn treble_lands_in_the_high_band() {
    let [l, m, h] = middle(&band_envelope(&sine(9000.0, 0.8, 2.0), 64));
    assert!((h - 1.0).abs() < 0.1, "high {h}");
    assert!(l < 0.05 && m < 0.1, "low {l} mid {m}");
}

#[test]
fn bands_share_the_waveform_scale() {
    // A quiet track still normalises to its own loudest sample, same as the waveform.
    let t = sine(50.0, 0.1, 2.0);
    let [l, _, _] = middle(&band_envelope(&t, 64));
    let full = waveform_envelope(&t, 64)[32][1];
    assert!((l - full).abs() < 0.1, "low {l} vs waveform {full}");
}
