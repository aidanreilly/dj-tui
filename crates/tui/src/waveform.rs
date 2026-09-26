//! Braille rasterising for the overview waveform.
//!
//! Each terminal cell is a 2 × 4 braille dot grid. The waveform is mirrored around a
//! horizontal centre line: half the rows grow upward, half grow downward.

const BRAILLE_BASE: u32 = 0x2800;
/// Dot bits indexed by [dot column][dot row] inside one cell.
const DOT_BITS: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];

/// Rasterise `envelope` (one value in 0..=1 per dot column) into `rows` lines of braille.
/// `rows` must be even and at least 2.
pub fn braille_rows(envelope: &[f32], rows: usize) -> Vec<String> {
    assert!(rows >= 2 && rows % 2 == 0, "waveform rows must be even, got {rows}");
    let cells = envelope.len().div_ceil(2);
    let half_dots = rows / 2 * 4;
    let mut grid = vec![vec![0u8; cells]; rows];

    let mut set = |dot_row: usize, col: usize| {
        grid[dot_row / 4][col / 2] |= DOT_BITS[col % 2][dot_row % 4];
    };
    for (col, &v) in envelope.iter().enumerate() {
        let h = (v.clamp(0.0, 1.0) * half_dots as f32).round() as usize;
        for k in 0..h {
            set(half_dots - 1 - k, col);
            set(half_dots + k, col);
        }
    }

    grid.into_iter()
        .map(|row| {
            row.into_iter()
                .map(|bits| char::from_u32(BRAILLE_BASE + bits as u32).unwrap_or(' '))
                .collect()
        })
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
