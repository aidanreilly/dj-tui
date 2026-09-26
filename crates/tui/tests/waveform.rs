//! Bar waveform rasterising (spec 3.2): half-cell-wide bars mirrored around the centre line.

use tui::waveform::{bar_rows, downsample_peaks};

/// Full-height bar, half-height stub below the cell centre, half-height stub above it.
const FULL: char = '▌';
const DOWN: char = '▖';
const UP: char = '▘';
const BLANK: char = ' ';

fn chars(rows: &[String]) -> Vec<Vec<char>> {
    rows.iter().map(|r| r.chars().collect()).collect()
}

#[test]
fn silence_draws_nothing() {
    let rows = chars(&bar_rows(&[0.0; 3], 4));
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().all(|r| r == &vec![BLANK, BLANK, BLANK]));
}

#[test]
fn full_scale_fills_every_row() {
    let rows = chars(&bar_rows(&[1.0, 1.0], 4));
    assert!(rows.iter().all(|r| r == &vec![FULL, FULL]));
}

#[test]
fn each_column_is_independent() {
    let rows = chars(&bar_rows(&[1.0, 0.0], 4));
    assert!(rows.iter().all(|r| r == &vec![FULL, BLANK]));
}

#[test]
fn one_envelope_value_per_column() {
    let rows = chars(&bar_rows(&[1.0, 1.0, 1.0], 2));
    assert_eq!(rows[0], vec![FULL, FULL, FULL]);
    assert_eq!(rows[1], vec![FULL, FULL, FULL]);
}

#[test]
fn half_scale_fills_the_cells_nearest_the_centre() {
    let rows = chars(&bar_rows(&[0.5], 4));
    assert_eq!(rows[0], vec![BLANK]);
    assert_eq!(rows[1], vec![FULL]);
    assert_eq!(rows[2], vec![FULL]);
    assert_eq!(rows[3], vec![BLANK]);
}

#[test]
fn smallest_step_is_a_half_height_stub_either_side_of_centre() {
    let rows = chars(&bar_rows(&[0.25], 4));
    assert_eq!(rows[0], vec![BLANK]);
    assert_eq!(rows[1], vec![DOWN]);
    assert_eq!(rows[2], vec![UP]);
    assert_eq!(rows[3], vec![BLANK]);
}

#[test]
fn an_odd_number_of_steps_tops_the_bar_with_a_stub() {
    let rows = chars(&bar_rows(&[0.75], 4));
    assert_eq!(rows[0], vec![DOWN]);
    assert_eq!(rows[1], vec![FULL]);
    assert_eq!(rows[2], vec![FULL]);
    assert_eq!(rows[3], vec![UP]);
}

#[test]
fn quiet_columns_still_draw_the_centre_stub() {
    let rows = chars(&bar_rows(&[0.01], 4));
    assert_eq!(rows[1], vec![DOWN]);
    assert_eq!(rows[2], vec![UP]);
}

#[test]
fn taller_panels_give_more_steps() {
    // Eight rows means four cells and so eight half-steps either side of the centre.
    let rows = chars(&bar_rows(&[0.5], 8));
    assert_eq!(rows.len(), 8);
    assert_eq!(rows[0], vec![BLANK]);
    assert_eq!(rows[1], vec![BLANK]);
    assert_eq!(rows[2], vec![FULL]);
    assert_eq!(rows[3], vec![FULL]);
    assert_eq!(rows[4], vec![FULL]);
    assert_eq!(rows[5], vec![FULL]);
    assert_eq!(rows[6], vec![BLANK]);
    assert_eq!(rows[7], vec![BLANK]);
}

#[test]
fn an_eighth_at_eight_rows_is_the_centre_stub() {
    let rows = chars(&bar_rows(&[0.125], 8));
    assert_eq!(rows[3], vec![DOWN]);
    assert_eq!(rows[4], vec![UP]);
    assert_eq!(rows[2], vec![BLANK]);
    assert_eq!(rows[5], vec![BLANK]);
}

#[test]
fn values_outside_zero_to_one_are_clamped() {
    assert_eq!(bar_rows(&[5.0, -1.0], 2), bar_rows(&[1.0, 0.0], 2));
}

#[test]
fn downsampling_takes_the_peak_of_each_bucket() {
    assert_eq!(downsample_peaks(&[0.0, 1.0, 0.0, 0.5], 2), vec![1.0, 0.5]);
}

#[test]
fn downsampling_empty_source_gives_zeros() {
    assert_eq!(downsample_peaks(&[], 3), vec![0.0; 3]);
}

#[test]
fn downsampling_to_a_larger_size_stretches() {
    assert_eq!(downsample_peaks(&[0.2, 0.8], 4), vec![0.2, 0.2, 0.8, 0.8]);
}

// --- Analysis points: downsampling and range extraction ---

use tui::waveform::{downsample_points, ranges};
use wave::WavePoint;

fn pt(peak: f32, bands: [f32; 3]) -> WavePoint {
    WavePoint {
        range: [-peak, peak],
        bands,
    }
}

#[test]
fn downsampling_points_averages_the_peak_of_each_bucket() {
    let src = vec![
        pt(0.0, [0.0; 3]),
        pt(1.0, [0.0; 3]),
        pt(0.5, [0.0; 3]),
        pt(0.5, [0.0; 3]),
    ];
    let out = downsample_points(&src, 2);
    assert_eq!(out.len(), 2);
    assert!((out[0].range[1] - 0.5).abs() < 1e-6);
    assert!((out[1].range[1] - 0.5).abs() < 1e-6);
}

#[test]
fn downsampling_points_averages_each_band_separately() {
    let src = vec![pt(1.0, [1.0, 0.0, 0.0]), pt(1.0, [0.0, 1.0, 0.0])];
    let out = downsample_points(&src, 1);
    assert!((out[0].bands[0] - 0.5).abs() < 1e-6);
    assert!((out[0].bands[1] - 0.5).abs() < 1e-6);
    assert_eq!(out[0].bands[2], 0.0);
}

#[test]
fn downsampling_an_empty_source_gives_silent_points() {
    assert_eq!(downsample_points(&[], 3), vec![WavePoint::default(); 3]);
}

#[test]
fn downsampling_to_zero_columns_gives_nothing() {
    // A terminal resize can hand the renderer a zero-width area.
    assert!(downsample_points(&[pt(1.0, [1.0; 3])], 0).is_empty());
}

#[test]
fn downsampling_points_to_a_larger_size_stretches() {
    let out = downsample_points(&[pt(1.0, [1.0, 0.0, 0.0])], 4);
    assert_eq!(out.len(), 4);
    assert!(out.iter().all(|p| (p.range[1] - 1.0).abs() < 1e-6));
}

#[test]
fn a_zero_sized_area_rasterises_without_panicking() {
    use tui::pixel::{Palette, WaveformBitmaps};
    let p = Palette::default();
    let src = vec![pt(1.0, [1.0; 3]); 8];
    assert_eq!(WaveformBitmaps::rasterize(&src, 0, 10, &p).normal().width(), 0);
    assert_eq!(
        WaveformBitmaps::rasterize(&src, 10, 0, &p).normal().height(),
        0
    );
}

#[test]
fn ranges_extracts_the_signed_pairs() {
    let src = vec![pt(0.25, [1.0; 3]), pt(0.75, [0.0; 3])];
    assert_eq!(ranges(&src), vec![[-0.25, 0.25], [-0.75, 0.75]]);
}

// --- Band colouring for the glyph fallback ---

use ratatui::style::Color;
use tui::pixel::Palette;
use tui::waveform::band_colors;

fn as_color(c: image::Rgba<u8>) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}

fn pt_row(bands: [f32; 3]) -> Vec<WavePoint> {
    vec![pt(1.0, bands)]
}

#[test]
fn band_colours_have_the_same_shape_as_the_glyph_rows() {
    let p = Palette::default();
    let src = vec![pt(1.0, [1.0, 0.5, 0.2]); 7];
    let colours = band_colors(&src, 8, &p);
    assert_eq!(colours.len(), 8);
    assert!(colours.iter().all(|row| row.len() == 7));
}

#[test]
fn the_topmost_band_wins_a_cell_where_two_overlap() {
    let p = Palette::default();
    // Bass at full scale over a quieter mid and high: the centre cells belong to the
    // high band, the outermost cell to the low band. Values chosen so the 1.8x and 3x
    // display gains do not push the upper bands to full scale.
    let colours = band_colors(&pt_row([1.0, 0.3, 0.15]), 8, &p);
    assert_eq!(colours[3][0], as_color(p.bands[2]), "cell above the line");
    assert_eq!(colours[4][0], as_color(p.bands[2]), "cell below the line");
    assert_eq!(colours[0][0], as_color(p.bands[0]), "outermost cell");
}

#[test]
fn a_quiet_high_band_does_not_reach_the_outer_cells() {
    let p = Palette::default();
    let colours = band_colors(&pt_row([1.0, 0.0, 0.05]), 8, &p);
    assert_eq!(colours[0][0], as_color(p.bands[0]));
}

#[test]
fn a_column_with_no_bands_falls_back_to_the_low_colour() {
    let p = Palette::default();
    let colours = band_colors(&pt_row([0.0, 0.0, 0.0]), 8, &p);
    assert_eq!(colours[3][0], as_color(p.bands[0]));
}

#[test]
fn zero_rows_or_no_points_give_nothing_to_draw() {
    let p = Palette::default();
    assert!(band_colors(&[], 8, &p).iter().all(|r| r.is_empty()));
    assert!(band_colors(&pt_row([1.0; 3]), 0, &p).is_empty());
}
