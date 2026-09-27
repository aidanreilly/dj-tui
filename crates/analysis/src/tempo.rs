//! Constant-tempo estimate and beat grid.
//!
//! An onset strength envelope comes from spectral flux. Each candidate tempo is scored by
//! the phase coherence of that envelope at the beat period: onsets that fall on a regular
//! grid add up in phase, anything else cancels. The phase of the winning sum gives the
//! grid offset directly.

use engine::Track;
use realfft::RealFftPlanner;
use std::f64::consts::TAU;

const WIN: usize = 1024;
const HOP: usize = 512;
const SEARCH_MIN: f64 = 50.0;
const SEARCH_MAX: f64 = 220.0;
const COARSE_STEP: f64 = 0.1;
const FINE_SPAN: f64 = 0.3;
const FINE_STEP: f64 = 0.002;
/// Below this share of perfectly regular onsets there is no usable beat.
const MIN_COHERENCE: f64 = 0.05;

/// Tempos are folded by halving or doubling into `[min, max)`, which settles half and
/// double time. The default suits most dance music.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoRange {
    pub min: f64,
    pub max: f64,
}

impl Default for TempoRange {
    fn default() -> Self {
        Self { min: 85.0, max: 175.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatGrid {
    pub bpm: f64,
    /// Time of the first beat, within the first beat period.
    pub first_beat_secs: f64,
}

impl BeatGrid {
    pub fn beat_secs(&self) -> f64 {
        60.0 / self.bpm
    }

    fn beats_at(&self, secs: f64) -> f64 {
        (secs - self.first_beat_secs) / self.beat_secs()
    }

    /// One-based bar and beat-in-bar, assuming 4/4 with bar 1 on the first beat.
    pub fn bar_and_beat(&self, secs: f64) -> (i64, i64) {
        let k = self.beats_at(secs).floor() as i64;
        (k.div_euclid(4) + 1, k.rem_euclid(4) + 1)
    }

    /// Position within the current beat, 0 on the beat.
    pub fn phase(&self, secs: f64) -> f64 {
        self.beats_at(secs).rem_euclid(1.0)
    }
}

/// Spectral flux, one value per hop.
fn onset_envelope(track: &Track) -> Vec<f64> {
    let n = track.frames();
    if n < WIN * 2 {
        return Vec::new();
    }
    let mono: Vec<f32> = (0..n).map(|i| { let (l, r) = track.frame_at(i as f64); (l + r) * 0.5 }).collect();
    let window: Vec<f32> = (0..WIN).map(|i| 0.5 - 0.5 * (TAU as f32 * i as f32 / WIN as f32).cos()).collect();
    let fft = RealFftPlanner::<f32>::new().plan_fft_forward(WIN);
    let mut input = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let mut prev = vec![0f32; spectrum.len()];
    let mut out = Vec::with_capacity(n / HOP);
    for start in (0..n - WIN).step_by(HOP) {
        for (d, (s, w)) in input.iter_mut().zip(mono[start..start + WIN].iter().zip(&window)) {
            *d = s * w;
        }
        fft.process(&mut input, &mut spectrum).expect("fft sizes match");
        let mut flux = 0.0;
        for (p, c) in prev.iter_mut().zip(&spectrum) {
            let m = (1.0 + 100.0 * c.norm()).ln();
            flux += (m - *p).max(0.0) as f64;
            *p = m;
        }
        out.push(flux);
    }
    if let Some(first) = out.first_mut() {
        *first = 0.0;
    }
    let mean = out.iter().sum::<f64>() / out.len() as f64;
    out.iter_mut().for_each(|v| *v -= mean);
    out
}

/// Sum of the envelope rotated at the beat period: magnitude and phase.
fn coherence(env: &[f64], fps: f64, bpm: f64) -> (f64, f64) {
    let step = TAU * bpm / 60.0 / fps;
    let (mut re, mut im) = (0.0, 0.0);
    let (mut c, mut s) = (1.0f64, 0.0f64);
    let (sc, ss) = (step.cos(), step.sin());
    for (t, &o) in env.iter().enumerate() {
        if t % 1024 == 0 {
            // Re-anchor the rotation now and then so rounding can't drift it.
            let a = step * t as f64;
            c = a.cos();
            s = a.sin();
        }
        re += o * c;
        im -= o * s;
        let nc = c * sc - s * ss;
        s = s * sc + c * ss;
        c = nc;
    }
    ((re * re + im * im).sqrt(), im.atan2(re))
}

fn best_in(env: &[f64], fps: f64, lo: f64, hi: f64, step: f64) -> (f64, f64) {
    let mut best = (lo, 0.0);
    let mut bpm = lo;
    while bpm <= hi {
        let (m, _) = coherence(env, fps, bpm);
        if m > best.1 {
            best = (bpm, m);
        }
        bpm += step;
    }
    best
}

pub fn detect_tempo(track: &Track, range: TempoRange) -> Option<BeatGrid> {
    let env = onset_envelope(track);
    let total: f64 = env.iter().map(|v| v.abs()).sum();
    if env.is_empty() || total <= 1e-9 {
        return None;
    }
    let fps = track.sample_rate() as f64 / HOP as f64;
    let (raw, raw_score) = best_in(&env, fps, SEARCH_MIN, SEARCH_MAX, COARSE_STEP);
    if raw_score / total < MIN_COHERENCE {
        return None;
    }

    // Harmonically related tempos score almost as well; take the strongest one in range.
    let mut choice: Option<(f64, f64)> = None;
    for m in [1.0, 2.0, 0.5, 3.0, 1.0 / 3.0, 4.0, 0.25, 1.5, 2.0 / 3.0] {
        let c = raw * m;
        if c < range.min || c >= range.max {
            continue;
        }
        let (bpm, score) = best_in(&env, fps, c - 2.0 * COARSE_STEP * m.max(1.0), c + 2.0 * COARSE_STEP * m.max(1.0), COARSE_STEP / 2.0);
        if score >= 0.6 * raw_score && choice.is_none_or(|(_, s)| score > s * 1.05) {
            choice = Some((bpm, score));
        }
    }
    let coarse = match choice {
        Some((bpm, _)) => bpm,
        None => {
            let mut b = raw;
            while b >= range.max {
                b /= 2.0;
            }
            while b < range.min {
                b *= 2.0;
            }
            b
        }
    };

    let (bpm, _) = best_in(&env, fps, coarse - FINE_SPAN, coarse + FINE_SPAN, FINE_STEP);
    let (_, phase) = coherence(&env, fps, bpm);
    let period_frames = 60.0 / bpm * fps;
    // The sum rotates as e^(-iθt), so an onset at frame t0 shows up at phase -θ·t0.
    let onset_frame = (-phase / TAU * period_frames).rem_euclid(period_frames);
    // A flux peak appears once a click is inside the analysis window; place the beat at
    // the window's midpoint.
    let secs = (onset_frame * HOP as f64 + WIN as f64 / 2.0) / track.sample_rate() as f64;
    let beat = 60.0 / bpm;
    Some(BeatGrid { bpm: (bpm * 100.0).round() / 100.0, first_beat_secs: secs.rem_euclid(beat) })
}
