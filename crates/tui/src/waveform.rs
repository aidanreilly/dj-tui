//! Signed waveform rasterising for the deck overview.

// Compatibility rasteriser for magnitude-only envelopes.
const FULL: char = '▌';
const DOWN: char = '▖';
const UP: char = '▘';

/// Rasterise signed `[minimum, maximum]` ranges as symmetrical peak bars around the center.
/// A nonlinear curve gives quieter sections more height contrast against loud peaks.
/// The left half of each terminal cell is the bar; its right half remains a one-pixel gap.
pub fn amplitude_rows(ranges: &[[f32; 2]], rows: usize) -> Vec<String> {
    assert!(rows > 0, "waveform must have at least one row");
    let height = rows * 2;
    let mut pixels = vec![vec![false; ranges.len()]; height];

    for (col, &[a, b]) in ranges.iter().enumerate() {
        let peak = a.abs().max(b.abs()).clamp(0.0, 1.0);
        // Emphasize the difference between lower-energy sections and strong peaks.
        let peak = peak.powf(1.5);
        // Silence should remain visually quiet rather than drawing a center pixel.
        if peak == 0.0 {
            continue;
        }

        let last = (height - 1) as f32;
        let min = -peak;
        let max = peak;
        let top = (((1.0 - max) * 0.5) * last).round() as usize;
        let bottom = (((1.0 - min) * 0.5) * last).round() as usize;
        for row in &mut pixels[top.min(height - 1)..=bottom.min(height - 1)] {
            row[col] = true;
        }
    }

    (0..rows)
        .map(|row| {
            (0..ranges.len())
                .map(
                    |col| match (pixels[row * 2][col], pixels[row * 2 + 1][col]) {
                        (true, true) => '▌',
                        (true, false) => '▘',
                        (false, true) => '▖',
                        (false, false) => ' ',
                    },
                )
                .collect()
        })
        .collect()
}

/// Resample signed ranges to exactly `n` columns as a centered average-peak envelope.
/// Averaging the source peaks in each screen column keeps long tracks from turning into
/// solid full-height bars when a column spans several seconds of audio.
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
            let peak = src[start..end]
                .iter()
                .map(|[min, max]| min.abs().max(max.abs()))
                .sum::<f32>()
                / (end - start) as f32;
            [-peak, peak]
        })
        .collect()
}

/// Rasterise a magnitude-only `envelope` (one value in 0..=1 per column) into bars.
/// `rows` must be even and at least 2.
pub fn bar_rows(envelope: &[f32], rows: usize) -> Vec<String> {
    assert!(
        rows >= 2 && rows.is_multiple_of(2),
        "waveform rows must be even, got {rows}"
    );
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

    grid.into_iter()
        .map(|row| row.into_iter().collect())
        .collect()
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

/// Reduce per-band peaks to `n` columns by averaging each bucket, the same way
/// [`downsample_ranges`] does, so band heights stay comparable with the waveform.
pub fn downsample_bands(src: &[[f32; 3]], n: usize) -> Vec<[f32; 3]> {
    if src.is_empty() {
        return vec![[0.0; 3]; n];
    }
    let len = src.len();
    (0..n)
        .map(|j| {
            let start = (j * len / n).min(len - 1);
            let end = ((j + 1) * len / n).clamp(start + 1, len);
            let n = (end - start) as f32;
            src[start..end].iter().fold([0.0f32; 3], |acc, b| {
                [acc[0] + b[0] / n, acc[1] + b[1] / n, acc[2] + b[2] / n]
            })
        })
        .collect()
}
