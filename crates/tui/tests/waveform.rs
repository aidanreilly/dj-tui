//! Bar waveform rasterising (spec 3.2): thin vertical lines mirrored around the centre line.

use tui::waveform::{bar_rows, downsample_peaks};

/// Full-height line, half-height stub below the cell centre, half-height stub above it.
const FULL: char = '│';
const DOWN: char = '╷';
const UP: char = '╵';
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
