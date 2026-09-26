//! Braille waveform rasterising (spec 3.2): mirrored around the centre line.

use tui::waveform::{braille_rows, downsample_peaks};

const BLANK: char = '\u{2800}';

fn chars(rows: &[String]) -> Vec<Vec<char>> {
    rows.iter().map(|r| r.chars().collect()).collect()
}

#[test]
fn silence_draws_blank_cells() {
    let rows = chars(&braille_rows(&[0.0; 4], 4));
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().all(|r| r == &vec![BLANK, BLANK]));
}

#[test]
fn full_scale_fills_every_dot() {
    let rows = chars(&braille_rows(&[1.0, 1.0], 4));
    assert!(rows.iter().all(|r| r == &vec!['⣿']));
}

#[test]
fn each_dot_column_is_independent() {
    let rows = chars(&braille_rows(&[1.0, 0.0], 4));
    assert!(rows.iter().all(|r| r == &vec!['⡇']));
}

#[test]
fn half_scale_fills_the_rows_nearest_the_centre() {
    let rows = chars(&braille_rows(&[0.5, 0.5], 4));
    assert_eq!(rows[0], vec![BLANK]);
    assert_eq!(rows[1], vec!['⣿']);
    assert_eq!(rows[2], vec!['⣿']);
    assert_eq!(rows[3], vec![BLANK]);
}

#[test]
fn smallest_step_lights_one_dot_either_side_of_centre() {
    let rows = chars(&braille_rows(&[0.125, 0.125], 4));
    assert_eq!(rows[1], vec!['⣀']);
    assert_eq!(rows[2], vec!['⠉']);
    assert_eq!(rows[0], vec![BLANK]);
}

#[test]
fn odd_column_count_leaves_right_half_of_last_cell_empty() {
    let rows = chars(&braille_rows(&[1.0, 1.0, 1.0], 2));
    assert_eq!(rows[0], vec!['⣿', '⡇']);
}

#[test]
fn values_outside_zero_to_one_are_clamped() {
    assert_eq!(braille_rows(&[5.0, -1.0], 2), braille_rows(&[1.0, 0.0], 2));
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
