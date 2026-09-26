//! Pixel-resolution waveform for terminals with a bitmap protocol (kitty graphics in Ghostty,
//! kitty, WezTerm). Everything here is pure: RGBA images in, RGBA images out.

use image::{Rgba, RgbaImage};
use tui::pixel::{playhead_x, Palette, PixelWaveform, WaveformBitmaps, PLAYHEAD_WIDTH};

const W: u32 = 200;
const H: u32 = 80;

fn pal() -> Palette {
    Palette::default()
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
    let ranges: Vec<[f32; 2]> = (0..100)
        .map(|i| {
            let p = (i as f32 / 100.0).sqrt();
            [-p, p]
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
    let ranges: Vec<[f32; 2]> = (0..100)
        .map(|i| {
            let p = 0.1 + 0.8 * i as f32 / 100.0;
            [-p, p]
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

#[test]
fn colour_runs_from_low_at_the_centre_to_high_at_the_peaks() {
    let p = pal();
    let b = WaveformBitmaps::rasterize(&flat(1.0, 10), 10, H, &p);
    let centre = b.normal().get_pixel(5, H / 2 - 1);
    let edge = b.normal().get_pixel(5, 0);
    assert!(
        dist(centre, &p.low) < dist(centre, &p.high),
        "centre {centre:?}"
    );
    assert!(dist(edge, &p.high) < dist(edge, &p.low), "edge {edge:?}");
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
