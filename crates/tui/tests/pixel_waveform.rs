//! Pixel-resolution waveform for terminals with a bitmap protocol (kitty graphics in Ghostty,
//! kitty, WezTerm). Everything here is pure: RGBA images in, RGBA images out.

use image::{Rgba, RgbaImage};
use tui::pixel::{playhead_x, Palette, PixelWaveform, WaveformBitmaps, PLAYHEAD_WIDTH};
use wave::{WavePoint, WaveformMode};

const W: u32 = 200;
const H: u32 = 80;

fn pal() -> Palette {
    Palette::default()
}

fn flat(peak: f32, n: usize) -> Vec<WavePoint> {
    vec![
        WavePoint {
            range: [-peak, peak],
            // Full-band content, so the height tests read the same picture they did
            // before bands existed.
            bands: [peak, peak, peak],
        };
        n
    ]
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

fn dist(a: &Rgba<u8>, b: &Rgba<u8>) -> i32 {
    (0..3).map(|i| (a[i] as i32 - b[i] as i32).abs()).sum()
}

// --- Rasterising ---

#[test]
fn images_have_the_requested_size() {
    let b = WaveformBitmaps::rasterize(&flat(0.5, 64), W, H, &pal());
    assert_eq!(b.normal().dimensions(), (W, H));
    assert_eq!(b.dimmed().dimensions(), (W, H));
}

#[test]
fn silence_draws_only_the_centre_line_on_a_transparent_background() {
    let b = WaveformBitmaps::rasterize(&flat(0.0, 64), W, H, &pal());
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
    let b = WaveformBitmaps::rasterize(&flat(1.0, 64), W, H, &pal());
    for x in [0, W / 2, W - 1] {
        assert_eq!(opaque_rows(b.normal(), x), H, "column {x}");
    }
}

#[test]
fn louder_columns_are_taller() {
    let mut ranges = flat(0.2, 50);
    ranges.extend(flat(0.8, 50));
    let b = WaveformBitmaps::rasterize(&ranges, 100, H, &pal());
    assert!(touched_rows(b.normal(), 75) > touched_rows(b.normal(), 25) + 10);
}

#[test]
fn bars_are_mirrored_around_the_centre() {
    let ranges: Vec<WavePoint> = (0..100)
        .map(|i| {
            let p = (i as f32 / 100.0).sqrt();
            WavePoint {
                range: [-p, p],
                bands: [p, p, p],
            }
        })
        .collect();
    let b = WaveformBitmaps::rasterize(&ranges, 100, H, &pal());
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
    let ranges: Vec<WavePoint> = (0..100)
        .map(|i| {
            let p = 0.1 + 0.8 * i as f32 / 100.0;
            WavePoint {
                range: [-p, p],
                bands: [p, p, p],
            }
        })
        .collect();
    let b = WaveformBitmaps::rasterize(&ranges, 100, H, &pal());
    let partial = (0..100).any(|x| {
        (0..H).any(|y| {
            let a = b.normal().get_pixel(x, y)[3];
            a > 0 && a < 255
        })
    });
    assert!(partial, "every edge pixel is fully on or off");
}

fn three_band() -> Palette {
    Palette::for_mode(WaveformMode::ThreeBand)
}

/// A single-column source, so a named x is unambiguous.
fn one(bands: [f32; 3], peak: f32) -> Vec<WavePoint> {
    vec![WavePoint {
        range: [-peak, peak],
        bands,
    }]
}

#[test]
fn bass_reaches_furthest_and_highs_form_a_core_at_the_centre() {
    let p = three_band();
    // Bass loud, highs quiet: exactly the case docs/spec.md:75 describes.
    let b = WaveformBitmaps::rasterize(&one([1.0, 0.5, 0.15], 1.0), 1, 64, &p);
    let img = b.normal();
    let centre = img.get_pixel(0, 32);
    let outer = img.get_pixel(0, 2);
    assert!(
        dist(centre, &p.bands[2]) < dist(centre, &p.bands[0]),
        "centre {centre:?} should be the high colour"
    );
    assert!(
        dist(outer, &p.bands[0]) < dist(outer, &p.bands[2]),
        "outer {outer:?} should be the low colour"
    );
}

#[test]
fn each_band_reaches_its_own_height() {
    let p = three_band();
    let only_low = WaveformBitmaps::rasterize(&one([1.0, 0.0, 0.0], 1.0), 1, 64, &p);
    let only_high = WaveformBitmaps::rasterize(&one([0.0, 0.0, 1.0], 1.0), 1, 64, &p);
    // Both bands are at full scale, so both fill the column.
    assert_eq!(opaque_rows(only_low.normal(), 0), 64);
    assert_eq!(opaque_rows(only_high.normal(), 0), 64);
    // A quiet band covers less than a loud one.
    let quiet = WaveformBitmaps::rasterize(&one([0.3, 0.0, 0.0], 1.0), 1, 64, &p);
    assert!(opaque_rows(quiet.normal(), 0) < 64);
}

#[test]
fn the_high_band_is_gained_up_so_it_stays_visible() {
    let p = three_band();
    // 0.15 of high content is typical of real music and must still draw.
    let b = WaveformBitmaps::rasterize(&one([1.0, 0.0, 0.15], 1.0), 1, 64, &p);
    let centre = b.normal().get_pixel(0, 32);
    assert!(
        dist(centre, &p.bands[2]) < dist(centre, &p.bands[0]),
        "centre {centre:?} lost the high band"
    );
}

#[test]
fn a_column_with_range_but_no_bands_still_draws() {
    // A DC offset or a filter transient can leave every band at zero.
    let p = three_band();
    let b = WaveformBitmaps::rasterize(&one([0.0, 0.0, 0.0], 1.0), 1, 64, &p);
    assert_eq!(opaque_rows(b.normal(), 0), 64);
    let pixel = b.normal().get_pixel(0, 10);
    assert!(
        dist(pixel, &p.bands[0]) < 16,
        "expected the low colour, got {pixel:?}"
    );
}

#[test]
fn silence_still_draws_only_the_centre_line() {
    let p = three_band();
    let b = WaveformBitmaps::rasterize(&one([0.0; 3], 0.0), 1, 64, &p);
    assert_eq!(opaque_rows(b.normal(), 0), 2);
}

#[test]
fn dimmed_copy_has_the_same_shape_and_is_darker() {
    let b = WaveformBitmaps::rasterize(&flat(0.7, 64), W, H, &pal());
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
    let b = WaveformBitmaps::rasterize(&[], 20, 10, &pal());
    assert_eq!(touched_rows(b.normal(), 3), 2, "just the centre line");
}

// --- Composing with the playhead ---

#[test]
fn compose_dims_the_played_part_and_draws_the_playhead() {
    let p = pal();
    let b = WaveformBitmaps::rasterize(&flat(1.0, 64), W, H, &p);
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
    let b = WaveformBitmaps::rasterize(&flat(0.5, 64), W, H, &pal());
    assert_eq!(&b.compose(None, &pal()), b.normal());
}

#[test]
fn playhead_at_the_right_edge_stays_inside_the_image() {
    let p = pal();
    let b = WaveformBitmaps::rasterize(&flat(0.5, 64), W, H, &p);
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
    assert!(pw.update(&r, (W, H), Some(10), &pal()).is_some());
    assert!(pw.update(&r, (W, H), Some(10), &pal()).is_none());
    assert_eq!(pw.rasterizations(), 1);
}

#[test]
fn moving_the_playhead_recomposes_without_rasterising_again() {
    let mut pw = PixelWaveform::default();
    let r = flat(0.5, 64);
    pw.update(&r, (W, H), Some(10), &pal());
    let img = pw.update(&r, (W, H), Some(11), &pal()).expect("new frame");
    assert_eq!(*img.get_pixel(11, 0), pal().playhead);
    assert_eq!(pw.rasterizations(), 1);
}

#[test]
fn a_new_track_or_size_rasterises_again() {
    let mut pw = PixelWaveform::default();
    pw.update(&flat(0.5, 64), (W, H), Some(0), &pal());
    assert!(pw.update(&flat(0.6, 64), (W, H), Some(0), &pal()).is_some());
    assert_eq!(pw.rasterizations(), 2);
    assert!(pw
        .update(&flat(0.6, 64), (W + 10, H), Some(0), &pal())
        .is_some());
    assert_eq!(pw.rasterizations(), 3);
}

#[test]
fn no_waveform_means_no_image() {
    let mut pw = PixelWaveform::default();
    assert!(pw.update(&[], (W, H), None, &pal()).is_none());
    assert!(!pw.has_image());
    pw.update(&flat(0.5, 8), (W, H), None, &pal());
    assert!(pw.has_image());
    pw.update(&[], (W, H), None, &pal());
    assert!(!pw.has_image(), "unloading clears the image");
}

#[test]
fn rgb_gives_one_colour_across_a_column() {
    let p = Palette::for_mode(WaveformMode::Rgb);
    let b = WaveformBitmaps::rasterize(&one([1.0, 0.0, 0.0], 1.0), 1, 64, &p);
    let img = b.normal();
    let top = img.get_pixel(0, 2);
    let centre = img.get_pixel(0, 32);
    assert_eq!(top, centre, "rgb columns are a single colour");
}

#[test]
fn rgb_maps_each_band_to_its_own_channel() {
    let p = Palette::for_mode(WaveformMode::Rgb);
    let bass = WaveformBitmaps::rasterize(&one([1.0, 0.0, 0.0], 1.0), 1, 64, &p);
    let pixel = *bass.normal().get_pixel(0, 32);
    assert!(pixel[0] > 200, "red channel was {}", pixel[0]);
    assert!(pixel[2] < 40, "blue channel was {}", pixel[2]);
}

#[test]
fn rgb_gives_an_evenly_balanced_column_the_middle_of_the_ramp() {
    // Equal energy in all three bands sits in the middle of the spectrum, which the
    // ramp paints green. docs/spec.md:72 puts the mids there.
    let p = Palette::for_mode(WaveformMode::Rgb);
    let b = WaveformBitmaps::rasterize(&one([1.0, 1.0, 1.0], 1.0), 1, 64, &p);
    let px = *b.normal().get_pixel(0, 32);
    assert!(px[1] > 200, "green channel was {}", px[1]);
    assert!(
        px[0] < 90 && px[2] < 90,
        "should not be washed out, got {px:?}"
    );
}

#[test]
fn rgb_height_comes_from_the_range_not_the_bands() {
    let p = Palette::for_mode(WaveformMode::Rgb);
    let quiet = WaveformBitmaps::rasterize(&one([1.0, 1.0, 1.0], 0.3), 1, 64, &p);
    let loud = WaveformBitmaps::rasterize(&one([1.0, 1.0, 1.0], 1.0), 1, 64, &p);
    assert!(opaque_rows(quiet.normal(), 0) < opaque_rows(loud.normal(), 0));
}

#[test]
fn blue_tints_toward_white_as_highs_rise() {
    let p = Palette::for_mode(WaveformMode::Blue);
    let dull = WaveformBitmaps::rasterize(&one([1.0, 0.2, 0.0], 1.0), 1, 64, &p);
    let bright = WaveformBitmaps::rasterize(&one([1.0, 0.2, 1.0], 1.0), 1, 64, &p);
    let a = *dull.normal().get_pixel(0, 32);
    let b = *bright.normal().get_pixel(0, 32);
    assert!(
        luma(&b) > luma(&a),
        "{} should exceed {}",
        luma(&b),
        luma(&a)
    );
}

#[test]
fn blue_height_comes_from_the_bands_combined() {
    let p = Palette::for_mode(WaveformMode::Blue);
    let thin = WaveformBitmaps::rasterize(&one([0.2, 0.0, 0.0], 1.0), 1, 64, &p);
    let full = WaveformBitmaps::rasterize(&one([1.0, 1.0, 1.0], 1.0), 1, 64, &p);
    assert!(opaque_rows(thin.normal(), 0) < opaque_rows(full.normal(), 0));
    assert_eq!(opaque_rows(full.normal(), 0), 64);
}

#[test]
fn every_mode_leaves_silence_as_the_centre_line_alone() {
    for mode in [
        WaveformMode::ThreeBand,
        WaveformMode::Rgb,
        WaveformMode::Blue,
    ] {
        let p = Palette::for_mode(mode);
        let b = WaveformBitmaps::rasterize(&one([0.0; 3], 0.0), 1, 64, &p);
        assert_eq!(opaque_rows(b.normal(), 0), 2, "{mode:?}");
    }
}

#[test]
fn changing_only_the_mode_produces_a_fresh_image() {
    let mut pw = PixelWaveform::default();
    let r = flat(0.6, 64);
    assert!(pw.update(&r, (W, H), Some(10), &three_band()).is_some());
    assert!(pw.update(&r, (W, H), Some(10), &three_band()).is_none());
    let rgb = Palette::for_mode(WaveformMode::Rgb);
    assert!(
        pw.update(&r, (W, H), Some(10), &rgb).is_some(),
        "a new mode must redraw"
    );
}

/// A two-column track: a bass-led column beside a treble-led one. `rgb` measures each
/// band against its own loudest column, so both are needed to express the behaviour.
fn two_columns() -> Vec<WavePoint> {
    vec![
        WavePoint {
            range: [-1.0, 1.0],
            bands: [1.0, 0.2, 0.1],
        },
        WavePoint {
            range: [-1.0, 1.0],
            bands: [0.2, 0.5, 0.6],
        },
    ]
}

#[test]
fn rgb_paints_a_bass_led_column_at_the_red_end() {
    let p = Palette::for_mode(WaveformMode::Rgb);
    let b = WaveformBitmaps::rasterize(&two_columns(), 2, 64, &p);
    let px = *b.normal().get_pixel(0, 32);
    assert!(px[0] > 200, "bass-led column should be red, got {px:?}");
    assert!(px[2] < 60, "and carry no blue, got {px:?}");
}

#[test]
fn rgb_paints_a_treble_led_column_at_the_blue_end() {
    let p = Palette::for_mode(WaveformMode::Rgb);
    let b = WaveformBitmaps::rasterize(&two_columns(), 2, 64, &p);
    let px = *b.normal().get_pixel(1, 32);
    assert!(px[2] > 200, "treble-led column should be blue, got {px:?}");
    assert!(px[0] < 60, "and carry no red, got {px:?}");
}

#[test]
fn rgb_hue_follows_the_balance_rather_than_the_level() {
    // Halving every band leaves the balance untouched, so the colour must not move.
    let p = Palette::for_mode(WaveformMode::Rgb);
    let loud = two_columns();
    let quiet: Vec<WavePoint> = loud
        .iter()
        .map(|w| WavePoint {
            range: w.range,
            bands: [w.bands[0] * 0.5, w.bands[1] * 0.5, w.bands[2] * 0.5],
        })
        .collect();
    let a = WaveformBitmaps::rasterize(&loud, 2, 64, &p);
    let b = WaveformBitmaps::rasterize(&quiet, 2, 64, &p);
    assert_eq!(a.normal().get_pixel(0, 32), b.normal().get_pixel(0, 32));
    assert_eq!(a.normal().get_pixel(1, 32), b.normal().get_pixel(1, 32));
}
