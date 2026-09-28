//! Pixel-resolution waveform for terminals with a bitmap protocol (kitty graphics in Ghostty,
//! kitty, WezTerm). Everything here is pure: RGBA images in, RGBA images out.

use image::{Rgba, RgbaImage};
use tui::pixel::{
    loop_columns, playhead_x, Palette, PixelWaveform, Wave, WaveformBitmaps, WaveformMode,
    PLAYHEAD_WIDTH,
};

const W: u32 = 200;
const H: u32 = 80;

fn pal() -> Palette {
    Palette::default()
}

/// Geometry tests use Blue mode without band data: one colour, so only shape matters.
fn blue(ranges: &[[f32; 2]]) -> Wave<'_> {
    Wave {
        ranges,
        bands: &[],
        mode: WaveformMode::Blue,
        warning: false,
        loop_cols: None,
        focused: true,
    }
}

fn flat(peak: f32, n: usize) -> Vec<[f32; 2]> {
    vec![[-peak, peak]; n]
}

fn opaque_rows(img: &RgbaImage, x: u32) -> u32 {
    (0..img.height())
        .filter(|&y| img.get_pixel(x, y)[3] == 255)
        .count() as u32
}

fn touched_rows(img: &RgbaImage, x: u32) -> u32 {
    (0..img.height())
        .filter(|&y| img.get_pixel(x, y)[3] > 0)
        .count() as u32
}

fn luma(p: &Rgba<u8>) -> u32 {
    p[0] as u32 + p[1] as u32 + p[2] as u32
}

// --- Rasterising ---

#[test]
fn images_have_the_requested_size() {
    let b = WaveformBitmaps::rasterize(&blue(&flat(0.5, 64)), W, H, &pal());
    assert_eq!(b.normal().dimensions(), (W, H));
    assert_eq!(b.dimmed().dimensions(), (W, H));
}

#[test]
fn silence_draws_only_the_centre_line_on_a_transparent_background() {
    let b = WaveformBitmaps::rasterize(&blue(&flat(0.0, 64)), W, H, &pal());
    let img = b.normal();
    for x in 0..W {
        for y in 0..H {
            let a = img.get_pixel(x, y)[3];
            // The line straddles the exact centre so the image stays mirror-symmetric.
            if y == H / 2 - 1 || y == H / 2 {
                assert!(a > 0, "centre line missing at x={x}");
            } else {
                assert_eq!(a, 0, "stray pixel at ({x},{y})");
            }
        }
    }
}

#[test]
fn full_scale_fills_the_whole_height() {
    let b = WaveformBitmaps::rasterize(&blue(&flat(1.0, 64)), W, H, &pal());
    for x in [0, W / 2, W - 1] {
        assert_eq!(opaque_rows(b.normal(), x), H, "column {x}");
    }
}

#[test]
fn louder_columns_are_taller() {
    let mut ranges = flat(0.2, 50);
    ranges.extend(flat(0.8, 50));
    let b = WaveformBitmaps::rasterize(&blue(&ranges), 100, H, &pal());
    assert!(touched_rows(b.normal(), 75) > touched_rows(b.normal(), 25) + 10);
}

#[test]
fn bars_are_mirrored_around_the_centre() {
    let ranges: Vec<[f32; 2]> = (0..100)
        .map(|i| {
            let p = (i as f32 / 100.0).sqrt();
            [-p, p]
        })
        .collect();
    let b = WaveformBitmaps::rasterize(&blue(&ranges), 100, H, &pal());
    let img = b.normal();
    for x in 0..100 {
        for y in 0..H / 2 {
            assert_eq!(
                img.get_pixel(x, y)[3],
                img.get_pixel(x, H - 1 - y)[3],
                "({x},{y})"
            );
        }
    }
}

#[test]
fn bar_edges_are_anti_aliased() {
    let ranges: Vec<[f32; 2]> = (0..100)
        .map(|i| {
            let p = 0.1 + 0.8 * i as f32 / 100.0;
            [-p, p]
        })
        .collect();
    let b = WaveformBitmaps::rasterize(&blue(&ranges), 100, H, &pal());
    let partial = (0..100).any(|x| {
        (0..H).any(|y| {
            let a = b.normal().get_pixel(x, y)[3];
            a > 0 && a < 255
        })
    });
    assert!(partial, "every edge pixel is fully on or off");
}

#[test]
fn three_band_image_uses_only_the_three_band_colours_inside() {
    let p = pal();
    let ranges = flat(1.0, 10);
    let bands = vec![[1.0, 0.5, 0.2]; 10];
    let wave = Wave {
        ranges: &ranges,
        bands: &bands,
        mode: WaveformMode::ThreeBand,
        warning: false,
        loop_cols: None,
        focused: true,
    };
    let b = WaveformBitmaps::rasterize(&wave, 10, H, &p);
    let mut seen = std::collections::HashSet::new();
    for y in 0..H {
        let px = *b.normal().get_pixel(5, y);
        if px[3] == 255 {
            assert!(
                p.three_band.contains(&px),
                "blended colour {px:?} at row {y}"
            );
            seen.insert(px);
        }
    }
    assert_eq!(seen.len(), 3, "all three bands visible");
    // Highs sit at the centre, lows at the outside.
    assert_eq!(*b.normal().get_pixel(5, H / 2), p.three_band[2]);
    assert_eq!(*b.normal().get_pixel(5, 1), p.three_band[0]);
}

#[test]
fn rgb_image_colours_a_bass_column_red() {
    let ranges = flat(0.8, 10);
    let bands = vec![[0.8, 0.0, 0.0]; 10];
    let wave = Wave {
        ranges: &ranges,
        bands: &bands,
        mode: WaveformMode::Rgb,
        warning: false,
        loop_cols: None,
        focused: true,
    };
    let b = WaveformBitmaps::rasterize(&wave, 10, H, &pal());
    let px = b.normal().get_pixel(5, H / 2);
    assert!(px[0] >= 200 && px[1] <= 40 && px[2] <= 40, "{px:?}");
}

#[test]
fn changing_mode_rasterises_again() {
    let mut pw = PixelWaveform::default();
    let ranges = flat(0.5, 8);
    let bands = vec![[0.5, 0.2, 0.1]; 8];
    pw.update(
        &Wave {
            ranges: &ranges,
            bands: &bands,
            mode: WaveformMode::ThreeBand,
            warning: false,
            loop_cols: None,
            focused: true,
        },
        (W, H),
        None,
        &pal(),
    );
    let again = pw.update(
        &Wave {
            ranges: &ranges,
            bands: &bands,
            mode: WaveformMode::Rgb,
            warning: false,
            loop_cols: None,
            focused: true,
        },
        (W, H),
        None,
        &pal(),
    );
    assert!(again.is_some());
    assert_eq!(pw.rasterizations(), 2);
}

#[test]
fn dimmed_copy_has_the_same_shape_and_is_darker() {
    let b = WaveformBitmaps::rasterize(&blue(&flat(0.7, 64)), W, H, &pal());
    for x in (0..W).step_by(7) {
        for y in 0..H {
            let (n, d) = (b.normal().get_pixel(x, y), b.dimmed().get_pixel(x, y));
            assert_eq!(n[3], d[3]);
            if n[3] > 0 {
                assert!(luma(d) < luma(n), "({x},{y}) not dimmer");
            }
        }
    }
}

#[test]
fn empty_waveform_is_silence() {
    let b = WaveformBitmaps::rasterize(&blue(&[]), 20, 10, &pal());
    assert_eq!(touched_rows(b.normal(), 3), 2, "just the centre line");
}

// --- Composing with the playhead ---

#[test]
fn compose_dims_the_played_part_and_draws_the_playhead() {
    let p = pal();
    let b = WaveformBitmaps::rasterize(&blue(&flat(1.0, 64)), W, H, &p);
    let img = b.compose(Some(100), &p);
    assert_eq!(img.get_pixel(10, 5), b.dimmed().get_pixel(10, 5));
    assert_eq!(img.get_pixel(150, 5), b.normal().get_pixel(150, 5));
    for x in 100..100 + PLAYHEAD_WIDTH {
        for y in 0..H {
            assert_eq!(*img.get_pixel(x, y), p.playhead, "playhead at ({x},{y})");
        }
    }
    assert_eq!(
        img.get_pixel(100 + PLAYHEAD_WIDTH, 5),
        b.normal().get_pixel(100 + PLAYHEAD_WIDTH, 5)
    );
}

#[test]
fn compose_without_a_playhead_is_the_normal_image() {
    let b = WaveformBitmaps::rasterize(&blue(&flat(0.5, 64)), W, H, &pal());
    assert_eq!(&b.compose(None, &pal()), b.normal());
}

#[test]
fn playhead_at_the_right_edge_stays_inside_the_image() {
    let p = pal();
    let b = WaveformBitmaps::rasterize(&blue(&flat(0.5, 64)), W, H, &p);
    let img = b.compose(Some(W - 1), &p);
    assert_eq!(*img.get_pixel(W - 1, 0), p.playhead);
}

#[test]
fn playhead_position_maps_time_to_pixels() {
    assert_eq!(playhead_x(0.0, 100.0, W), Some(0));
    assert_eq!(playhead_x(50.0, 100.0, W), Some(100));
    assert_eq!(playhead_x(100.0, 100.0, W), Some(W - PLAYHEAD_WIDTH));
    assert_eq!(playhead_x(500.0, 100.0, W), Some(W - PLAYHEAD_WIDTH));
    assert_eq!(playhead_x(5.0, 0.0, W), None);
}

// --- Deciding when to send a new image ---

#[test]
fn first_frame_produces_an_image_and_identical_frames_do_not() {
    let mut pw = PixelWaveform::default();
    let r = flat(0.5, 64);
    assert!(pw.update(&blue(&r), (W, H), Some(10), &pal()).is_some());
    assert!(pw.update(&blue(&r), (W, H), Some(10), &pal()).is_none());
    assert_eq!(pw.rasterizations(), 1);
}

#[test]
fn moving_the_playhead_recomposes_without_rasterising_again() {
    let mut pw = PixelWaveform::default();
    let r = flat(0.5, 64);
    pw.update(&blue(&r), (W, H), Some(10), &pal());
    let img = pw
        .update(&blue(&r), (W, H), Some(11), &pal())
        .expect("new frame");
    assert_eq!(*img.get_pixel(11, 0), pal().playhead);
    assert_eq!(pw.rasterizations(), 1);
}

#[test]
fn a_new_track_or_size_rasterises_again() {
    let mut pw = PixelWaveform::default();
    pw.update(&blue(&flat(0.5, 64)), (W, H), Some(0), &pal());
    assert!(pw
        .update(&blue(&flat(0.6, 64)), (W, H), Some(0), &pal())
        .is_some());
    assert_eq!(pw.rasterizations(), 2);
    assert!(pw
        .update(&blue(&flat(0.6, 64)), (W + 10, H), Some(0), &pal())
        .is_some());
    assert_eq!(pw.rasterizations(), 3);
}

#[test]
fn no_waveform_means_no_image() {
    let mut pw = PixelWaveform::default();
    assert!(pw.update(&blue(&[]), (W, H), None, &pal()).is_none());
    assert!(!pw.has_image());
    pw.update(&blue(&flat(0.5, 8)), (W, H), None, &pal());
    assert!(pw.has_image());
    pw.update(&blue(&[]), (W, H), None, &pal());
    assert!(!pw.has_image(), "unloading clears the image");
}

#[test]
fn end_warning_tints_the_unplayed_part_red() {
    let p = pal();
    let b = WaveformBitmaps::rasterize(&blue(&flat(1.0, 64)), W, H, &p);
    let img = b.compose_with(Some(100), true, None, &p);
    let unplayed = img.get_pixel(150, H / 2);
    assert!(
        unplayed[0] > 150 && unplayed[0] > unplayed[2],
        "{unplayed:?}"
    );
    assert_eq!(img.get_pixel(10, H / 2), b.dimmed().get_pixel(10, H / 2));
}

#[test]
fn toggling_the_warning_sends_a_new_image() {
    let mut pw = PixelWaveform::default();
    let r = flat(0.5, 64);
    pw.update(&blue(&r), (W, H), Some(10), &pal());
    let w = Wave {
        warning: true,
        ..blue(&r)
    };
    assert!(pw.update(&w, (W, H), Some(10), &pal()).is_some());
    assert_eq!(pw.rasterizations(), 1, "recompose only");
}

#[test]
fn loop_columns_map_the_loop_onto_the_overview() {
    assert_eq!(
        loop_columns(Some((30.0, 60.0)), 120.0, 200),
        Some((50, 100))
    );
    assert_eq!(
        loop_columns(Some((-5.0, 500.0)), 120.0, 200),
        Some((0, 200)),
        "a loop reaching past either end stops at the edge"
    );
    assert_eq!(loop_columns(None, 120.0, 200), None);
    assert_eq!(loop_columns(Some((0.0, 10.0)), 0.0, 200), None, "no track");
    assert_eq!(loop_columns(Some((0.0, 10.0)), 120.0, 0), None, "no width");
}

#[test]
fn a_loop_tints_its_region_and_draws_a_line_at_each_edge() {
    let p = pal();
    let b = WaveformBitmaps::rasterize(&blue(&flat(1.0, 64)), W, H, &p);
    let plain = b.compose_with(None, false, None, &p);
    let looped = b.compose_with(None, false, Some((50, 100)), &p);

    let y = H / 2;
    assert_eq!(
        looped.get_pixel(20, y),
        plain.get_pixel(20, y),
        "outside the loop nothing changes"
    );
    assert_ne!(
        looped.get_pixel(70, y),
        plain.get_pixel(70, y),
        "inside the loop the waveform is tinted"
    );
    let edge = looped.get_pixel(50, y);
    assert_eq!(edge, &p.loop_region, "the loop in point is a solid line");
    assert_eq!(
        looped.get_pixel(99, y),
        &p.loop_region,
        "and so is the last column of the loop"
    );
    assert_eq!(
        looped.get_pixel(100, y),
        plain.get_pixel(100, y),
        "the column after the loop is outside it"
    );
}

#[test]
fn the_loop_tint_keeps_the_waveform_shape() {
    let p = pal();
    let b = WaveformBitmaps::rasterize(&blue(&flat(0.4, 64)), W, H, &p);
    let looped = b.compose_with(None, false, Some((0, W)), &p);
    let plain = b.compose_with(None, false, None, &p);
    for x in [10, 60, 150] {
        assert_eq!(
            touched_rows(&looped, x),
            touched_rows(&plain, x),
            "the tint colours what is drawn, it does not fill the panel"
        );
    }
}

#[test]
fn changing_the_loop_sends_a_new_image_without_rasterising_again() {
    let mut pw = PixelWaveform::default();
    let r = flat(0.5, 64);
    pw.update(&blue(&r), (W, H), Some(10), &pal());
    let looped = Wave {
        loop_cols: Some((20, 80)),
        ..blue(&r)
    };
    assert!(pw.update(&looped, (W, H), Some(10), &pal()).is_some());
    assert!(
        pw.update(&looped, (W, H), Some(10), &pal()).is_none(),
        "the same loop needs no new image"
    );
    assert!(pw.update(&blue(&r), (W, H), Some(10), &pal()).is_some());
    assert_eq!(pw.rasterizations(), 1, "recompose only");
}

#[test]
fn the_loop_still_shows_in_the_part_already_played() {
    let p = pal();
    let b = WaveformBitmaps::rasterize(&blue(&flat(1.0, 64)), W, H, &p);
    // The playhead sits past the loop, so the whole loop is in the dimmed part.
    let img = b.compose_with(Some(150), false, Some((50, 100)), &p);
    let y = H / 2;
    assert_eq!(img.get_pixel(50, y), &p.loop_region, "the in point line");
    assert_eq!(img.get_pixel(99, y), &p.loop_region, "the out point line");
    assert_ne!(
        img.get_pixel(70, y),
        b.dimmed().get_pixel(70, y),
        "the region inside keeps its tint under the played dimming"
    );
    assert_eq!(
        img.get_pixel(150, y),
        &p.playhead,
        "the playhead stays on top"
    );
}

#[test]
fn an_unfocused_decks_waveform_is_rasterized_darker() {
    let p = Palette::default();
    let ranges = flat(1.0, 10);
    let lit = WaveformBitmaps::rasterize(&blue(&ranges), 10, H, &p);
    let grey = WaveformBitmaps::rasterize(
        &Wave {
            focused: false,
            ..blue(&ranges)
        },
        10,
        H,
        &p,
    );

    let brightness = |img: &image::RgbaImage| -> u32 {
        img.pixels()
            .map(|q| q[0] as u32 + q[1] as u32 + q[2] as u32)
            .sum()
    };
    let (a, b) = (brightness(lit.normal()), brightness(grey.normal()));
    assert!(a > 0, "the focused deck drew something");
    assert!(
        b * 2 < a,
        "the unfocused deck is clearly darker: {b} against {a}"
    );
}

#[test]
fn moving_focus_redraws_the_image() {
    let p = Palette::default();
    let ranges = flat(1.0, 10);
    let mut pw = PixelWaveform::default();
    assert!(pw.update(&blue(&ranges), (10, H), None, &p).is_some());
    assert_eq!(pw.rasterizations(), 1);
    // Focus is part of what the bitmap depends on, so losing it has to redraw.
    assert!(pw
        .update(
            &Wave {
                focused: false,
                ..blue(&ranges)
            },
            (10, H),
            None,
            &p
        )
        .is_some());
    assert_eq!(pw.rasterizations(), 2);
}
