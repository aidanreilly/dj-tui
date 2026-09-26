//! Signed waveform rasterising for the deck overview.

use crate::pixel::{band_half_steps, Palette};
use ratatui::style::Color;
use wave::WavePoint;

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
        for row in top.min(height - 1)..=bottom.min(height - 1) {
            pixels[row][col] = true;
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

/// Resample points to exactly `n` columns, averaging within each column.
///
/// Averaging rather than taking the peak keeps a long track from turning into solid
/// full-height bars when one column spans several seconds of audio.
pub fn downsample_points(src: &[WavePoint], n: usize) -> Vec<WavePoint> {
    if n == 0 {
        return Vec::new();
    }
    if src.is_empty() {
        return vec![WavePoint::default(); n];
    }
    let len = src.len();
    (0..n)
        .map(|j| {
            let start = (j * len / n).min(len - 1);
            let end = ((j + 1) * len / n).clamp(start + 1, len);
            let bucket = &src[start..end];
            let count = bucket.len() as f32;
            let peak = bucket
                .iter()
                .map(|p| p.range[0].abs().max(p.range[1].abs()))
                .sum::<f32>()
                / count;
            let mut bands = [0.0f32; 3];
            for p in bucket {
                for (acc, v) in bands.iter_mut().zip(p.bands) {
                    *acc += v;
                }
            }
            for b in &mut bands {
                *b /= count;
            }
            WavePoint {
                range: [-peak, peak],
                bands,
            }
        })
        .collect()
}

/// The signed pairs alone, for the glyph rasteriser.
pub fn ranges(points: &[WavePoint]) -> Vec<[f32; 2]> {
    points.iter().map(|p| p.range).collect()
}

/// Rasterise a magnitude-only `envelope` (one value in 0..=1 per column) into bars.
/// `rows` must be even and at least 2.
pub fn bar_rows(envelope: &[f32], rows: usize) -> Vec<String> {
    assert!(
        rows >= 2 && rows % 2 == 0,
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

/// One colour per cell, matching the geometry `amplitude_rows` draws.
///
/// A cell holds one foreground colour and two half-block pixels, so the band is chosen
/// from the cell's outer pixel: the topmost band reaching that far out wins, which puts
/// bass at the edges and highs near the centre line.
pub fn band_colors(points: &[WavePoint], rows: usize, palette: &Palette) -> Vec<Vec<Color>> {
    if rows == 0 {
        return Vec::new();
    }
    let height = rows * 2;
    let centre = (height - 1) as f32 * 0.5;
    let low = rgba_to_color(palette.bands[0]);

    (0..rows)
        .map(|row| {
            // The cell's two half-block pixels. A band counts for this cell when it
            // reaches into either of them, which is what docs/spec.md:75 asks for: a
            // quiet band spanning only the pixels beside the centre line still colours
            // the two centre cells.
            let (first, last_pixel) = (row * 2, row * 2 + 1);
            points
                .iter()
                .map(|p| {
                    let mut colour = low;
                    for band in 0..3 {
                        let frac = band_half_steps(p.bands[band], band);
                        if frac <= 0.0 {
                            continue;
                        }
                        let half = frac * centre;
                        let top = (centre - half).round() as usize;
                        let bottom = (centre + half).round() as usize;
                        if first <= bottom && last_pixel >= top {
                            colour = rgba_to_color(palette.bands[band]);
                        }
                    }
                    colour
                })
                .collect()
        })
        .collect()
}

fn rgba_to_color(c: image::Rgba<u8>) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}
