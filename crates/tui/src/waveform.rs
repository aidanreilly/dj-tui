//! Bar rasterising for the overview waveform.
//!
//! One bar per terminal cell column, filling the left half of the cell so that bars sit
//! close together with a narrow gap. The waveform is mirrored around a horizontal centre
//! line: half the rows grow upward, half grow downward. Each cell resolves two steps, a
//! full-height bar and a quadrant stub on the side facing the centre.

/// Full-height bar, left half of the cell.
const FULL: char = '▌';
/// Half-height stub in the lower part of a cell, for a bar growing upward.
const DOWN: char = '▖';
/// Half-height stub in the upper part of a cell, for a bar growing downward.
const UP: char = '▘';

/// Rasterise `envelope` (one value in 0..=1 per cell column) into `rows` lines of bars.
/// `rows` must be even and at least 2.
pub fn bar_rows(envelope: &[f32], rows: usize) -> Vec<String> {
    assert!(rows >= 2 && rows % 2 == 0, "waveform rows must be even, got {rows}");
    let half_cells = rows / 2;
    let half_steps = half_cells * 2;
    let mut grid = vec![vec![' '; envelope.len()]; rows];

    for (col, &v) in envelope.iter().enumerate() {
        let v = v.clamp(0.0, 1.0);
        let mut steps = (v * half_steps as f32).round() as usize;
        // Anything audible keeps its place on the centre line rather than dropping out.
        if steps == 0 && v > 0.0 {
            steps = 1;
        }
        for k in 0..steps {
            let cell = k / 2;
            let tops_out_here = k % 2 == 0 && k + 1 == steps;
            grid[half_cells - 1 - cell][col] = if tops_out_here { DOWN } else { FULL };
            grid[half_cells + cell][col] = if tops_out_here { UP } else { FULL };
        }
    }

    grid.into_iter().map(|row| row.into_iter().collect()).collect()
}

/// Reduce (or stretch) `src` to exactly `n` values, keeping the peak of each bucket.
pub fn downsample_peaks(src: &[f32], n: usize) -> Vec<f32> {
    if src.is_empty() {
        return vec![0.0; n];
    }
    let len = src.len();
    (0..n)
        .map(|j| {
            let start = (j * len / n).min(len - 1);
            let end = ((j + 1) * len / n).clamp(start + 1, len);
            src[start..end].iter().copied().fold(0.0, f32::max)
        })
        .collect()
}
