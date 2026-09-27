//! Colour rules for the three waveform modes, following Pioneer's CDJ-3000 and rekordbox:
//! RGB blends red lows, green mids and blue highs; 3-Band layers blue lows, amber mids and
//! white highs as separate shapes; Blue brightens toward white as highs rise.

use image::Rgba;
use tui::pixel::{colour_at, Palette, WaveformMode::*};
use tui::waveform::downsample_bands;

fn p() -> Palette {
    Palette::default()
}

fn luma(c: Rgba<u8>) -> u32 {
    c[0] as u32 + c[1] as u32 + c[2] as u32
}

#[test]
fn rgb_bass_is_red() {
    let c = colour_at(Rgb, 1.0, [1.0, 0.0, 0.0], 0.0, &p()).unwrap();
    assert!(c[0] >= 200 && c[1] <= 40 && c[2] <= 40, "{c:?}");
}

#[test]
fn rgb_bass_and_mids_together_are_yellow() {
    let c = colour_at(Rgb, 1.0, [0.8, 0.8, 0.0], 0.0, &p()).unwrap();
    assert!(c[0] >= 200 && c[1] >= 200 && c[2] <= 40, "{c:?}");
}

#[test]
fn rgb_highs_are_blue() {
    let c = colour_at(Rgb, 1.0, [0.0, 0.0, 0.6], 0.0, &p()).unwrap();
    assert!(c[2] >= 200 && c[0] <= 40 && c[1] <= 40, "{c:?}");
}

#[test]
fn rgb_keeps_small_components_small_for_clear_hues() {
    // Mostly bass with a little top end should still read as red, not pink.
    let c = colour_at(Rgb, 1.0, [1.0, 0.1, 0.3], 0.0, &p()).unwrap();
    assert!(c[0] >= 200 && c[1] <= 40 && c[2] <= 40, "{c:?}");
}

#[test]
fn rgb_and_blue_are_empty_outside_the_envelope() {
    assert!(colour_at(Rgb, 0.5, [0.5, 0.0, 0.0], 0.9, &p()).is_none());
    assert!(colour_at(Blue, 0.5, [0.5, 0.0, 0.0], 0.9, &p()).is_none());
}

#[test]
fn blue_mode_brightens_with_highs_and_stays_blue() {
    let lows = colour_at(Blue, 1.0, [1.0, 0.0, 0.0], 0.0, &p()).unwrap();
    let highs = colour_at(Blue, 1.0, [0.0, 0.0, 1.0], 0.0, &p()).unwrap();
    assert!(luma(highs) > luma(lows) + 150, "{lows:?} {highs:?}");
    for c in [lows, highs] {
        assert!(c[2] >= c[0] && c[2] >= c[1], "not blue: {c:?}");
    }
}

#[test]
fn three_band_layers_highs_over_mids_over_lows() {
    let pal = p();
    let bands = [1.0, 0.5, 0.2];
    // Heights use the same 1.5 power curve as the glyph renderer: 0.2 -> 0.089, 0.5 -> 0.354.
    assert_eq!(
        colour_at(ThreeBand, 1.0, bands, 0.05, &pal),
        Some(pal.three_band[2])
    );
    assert_eq!(
        colour_at(ThreeBand, 1.0, bands, 0.2, &pal),
        Some(pal.three_band[1])
    );
    assert_eq!(
        colour_at(ThreeBand, 1.0, bands, 0.6, &pal),
        Some(pal.three_band[0])
    );
    assert_eq!(colour_at(ThreeBand, 1.0, [0.5, 0.2, 0.1], 0.6, &pal), None);
}

#[test]
fn three_band_colours_are_blue_amber_white() {
    let [low, mid, high] = p().three_band;
    assert!(low[2] > 200 && low[0] < 80, "low {low:?}");
    assert!(mid[0] > 200 && mid[1] > 120 && mid[2] < 60, "mid {mid:?}");
    assert!(high.0[..3].iter().all(|&c| c > 220), "high {high:?}");
}

#[test]
fn without_band_data_every_mode_still_draws() {
    for mode in [ThreeBand, Rgb, Blue] {
        assert!(
            colour_at(mode, 0.8, [0.0; 3], 0.1, &p()).is_some(),
            "{mode:?}"
        );
    }
}

#[test]
fn downsampling_bands_averages_like_the_waveform_does() {
    // Averaging matches `downsample_ranges`, so band heights stay on the waveform's scale.
    // Taking the peak instead let a single hi-hat turn a whole wide column white.
    let src = [
        [0.1, 0.9, 0.0],
        [0.7, 0.2, 0.3],
        [0.0, 0.0, 1.0],
        [0.2, 0.1, 0.0],
    ];
    let got = downsample_bands(&src, 2);
    let want = [[0.4, 0.55, 0.15], [0.1, 0.05, 0.5]];
    for (g, w) in got.iter().zip(want) {
        for i in 0..3 {
            assert!((g[i] - w[i]).abs() < 1e-6, "{got:?}");
        }
    }
    assert_eq!(downsample_bands(&[], 3), vec![[0.0; 3]; 3]);
}
