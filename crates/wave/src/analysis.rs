//! Per-bucket signed range and three-band RMS for the overview waveform.

use crate::Biquad;

/// Overview resolution. `docs/spec.md` asks for about twenty points a second.
pub const POINTS_PER_SECOND: u32 = 20;

/// Low to mid crossover, matching the isolator EQ.
pub const LOW_HZ: f32 = 250.0;
/// Mid to high crossover, matching the isolator EQ.
pub const HIGH_HZ: f32 = 2_500.0;

/// One time bucket of the overview waveform.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WavePoint {
    /// Signed minimum and maximum sample, divided by the track's loudest absolute sample.
    pub range: [f32; 2],
    /// Low, mid and high RMS, all divided by the largest band RMS in the track.
    pub bands: [f32; 3],
}

/// Analyse planar stereo at its own sample rate.
///
/// Runs before resampling, so the result does not depend on the session rate and one
/// cache file serves every session. The three bands share one divisor, which is what
/// keeps a quiet cymbal looking quiet next to a loud kick.
pub fn analyse(left: &[f32], right: &[f32], sample_rate: u32) -> Vec<WavePoint> {
    let frames = left.len().min(right.len());
    if frames == 0 || sample_rate == 0 {
        return Vec::new();
    }
    let points = ((frames as u64 * POINTS_PER_SECOND as u64) / sample_rate as u64).max(1) as usize;

    let sr = sample_rate as f32;
    let mut low = Biquad::low_pass(LOW_HZ, sr);
    let mut mid_hp = Biquad::high_pass(LOW_HZ, sr);
    let mut mid_lp = Biquad::low_pass(HIGH_HZ, sr);
    let mut high = Biquad::high_pass(HIGH_HZ, sr);

    let mut out = Vec::with_capacity(points);
    for j in 0..points {
        let start = (j * frames / points).min(frames - 1);
        let end = ((j + 1) * frames / points).clamp(start + 1, frames);
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut sums = [0.0f64; 3];
        for i in start..end {
            let (l, r) = (left[i], right[i]);
            min = min.min(l).min(r);
            max = max.max(l).max(r);
            let mono = 0.5 * (l + r);
            let bands = [
                low.process(mono),
                mid_lp.process(mid_hp.process(mono)),
                high.process(mono),
            ];
            for (sum, v) in sums.iter_mut().zip(bands) {
                *sum += (v * v) as f64;
            }
        }
        let n = (end - start) as f64;
        out.push(WavePoint {
            range: [min, max],
            bands: [
                (sums[0] / n).sqrt() as f32,
                (sums[1] / n).sqrt() as f32,
                (sums[2] / n).sqrt() as f32,
            ],
        });
    }

    let peak = out
        .iter()
        .flat_map(|p| [p.range[0].abs(), p.range[1].abs()])
        .fold(0.0f32, f32::max);
    let band_peak = out.iter().flat_map(|p| p.bands).fold(0.0f32, f32::max);
    for p in &mut out {
        if peak > 0.0 {
            p.range[0] /= peak;
            p.range[1] /= peak;
        }
        if band_peak > 0.0 {
            for b in &mut p.bands {
                *b /= band_peak;
            }
        }
    }
    out
}
