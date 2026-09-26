//! Signed waveform rasterising for the deck overview.

// Compatibility rasteriser for magnitude-only envelopes.
const FULL: char = '▌';
const DOWN: char = '▖';
const UP: char = '▘';

/// Rasterise signed `[minimum, maximum]` ranges as thin bars around the center line.
/// The left half of each terminal cell is the bar; its right half remains a one-pixel gap.
/// Upper and lower quadrant glyphs add vertical detail at each bar's ends.
pub fn amplitude_rows(ranges: &[[f32; 2]], rows: usize) -> Vec<String> {
    assert!(rows > 0, "waveform must have at least one row");
    let height = rows * 2;
    let mut pixels = vec![vec![false; ranges.len()]; height];

    for (col, &[a, b]) in ranges.iter().enumerate() {
        let min = a.clamp(-1.0, 1.0).min(b.clamp(-1.0, 1.0));
        let max = a.clamp(-1.0, 1.0).max(b.clamp(-1.0, 1.0));
        // Silence should remain visually quiet rather than drawing a center pixel.
        if min == 0.0 && max == 0.0 {
            continue;
        }

        let last = (height - 1) as f32;
        let top = (((1.0 - max) * 0.5) * last).round() as usize;
        let bottom = (((1.0 - min) * 0.5) * last).round() as usize;
        for row in top.min(height - 1)..=bottom.min(height - 1) {
            pixels[row][col] = true;
        }
    }

    (0..rows)
        .map(|row| {
            (0..ranges.len())
                .map(|col| match (pixels[row * 2][col], pixels[row * 2 + 1][col]) {
                    (true, true) => '▌',
                    (true, false) => '▘',
                    (false, true) => '▖',
                    (false, false) => ' ',
                })
                .collect()
        })
        .collect()
}

/// Resample signed ranges to exactly `n` columns, retaining each bucket's extrema.
pub fn downsample_ranges(src: &[[f32; 2]], n: usize) -> Vec<[f32; 2]> {
    if n == 0 {
        return Vec::new();
    }
    if src.is_empty() {
        return vec![[0.0, 0.0]; n];
    }
    let len = src.len();
    (0..n)
        .map(|j| {
            let start = (j * len / n).min(len - 1);
            let end = ((j + 1) * len / n).clamp(start + 1, len);
            src[start..end]
                .iter()
                .fold([f32::INFINITY, f32::NEG_INFINITY], |acc, range| {
                    [acc[0].min(range[0]), acc[1].max(range[1])]
                })
        })
        .collect()
}

/// Rasterise a magnitude-only `envelope` (one value in 0..=1 per column) into bars.
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
