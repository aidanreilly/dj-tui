//! Channel DSP: isolator EQ, one-knob filter and trim (spec 3.3).
//!
//! All processors work in place on interleaved stereo, keep their state in fixed-size
//! structs, and never allocate. Parameter changes are smoothed to avoid zipper noise.

use std::f64::consts::TAU;

/// Isolator crossover points.
pub const LOW_MID_HZ: f64 = 250.0;
pub const MID_HIGH_HZ: f64 = 2500.0;
pub const EQ_MAX_DB: f32 = 6.0;
pub const EQ_MIN_DB: f32 = -40.0;
pub const TRIM_RANGE_DB: f32 = 12.0;
/// Filter knob positions closer to centre than this bypass the filter entirely.
pub const FILTER_DEAD_ZONE: f32 = 0.05;
const FILTER_LP_MIN_HZ: f64 = 60.0;
const FILTER_LP_MAX_HZ: f64 = 20_000.0;
const FILTER_HP_MIN_HZ: f64 = 20.0;
const FILTER_HP_MAX_HZ: f64 = 8_000.0;
const FILTER_Q: f64 = 0.9;
/// Filter coefficients are recomputed every this many frames.
const FILTER_SUBBLOCK: usize = 32;
/// Time constant for gain smoothing.
const SMOOTH_SECS: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EqBand {
    Low,
    Mid,
    High,
}

impl EqBand {
    fn index(self) -> usize {
        self as usize
    }
}

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

fn smoothing_coeff(fs: f32, secs: f32) -> f32 {
    1.0 - (-1.0 / (secs * fs)).exp()
}

/// Second-order section, transposed direct form II, f64 state.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    fn lowpass(fs: f64, f: f64, q: f64) -> Self {
        let mut b = Self::default();
        b.set_lowpass(fs, f, q);
        b
    }

    fn highpass(fs: f64, f: f64, q: f64) -> Self {
        let mut b = Self::default();
        b.set_highpass(fs, f, q);
        b
    }

    fn set_coeffs(&mut self, b: [f64; 3], a: [f64; 3]) {
        self.b0 = b[0] / a[0];
        self.b1 = b[1] / a[0];
        self.b2 = b[2] / a[0];
        self.a1 = a[1] / a[0];
        self.a2 = a[2] / a[0];
    }

    fn set_lowpass(&mut self, fs: f64, f: f64, q: f64) {
        let (c, s) = ((TAU * f / fs).cos(), (TAU * f / fs).sin());
        let alpha = s / (2.0 * q);
        self.set_coeffs(
            [(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        );
    }

    fn set_highpass(&mut self, fs: f64, f: f64, q: f64) {
        let (c, s) = ((TAU * f / fs).cos(), (TAU * f / fs).sin());
        let alpha = s / (2.0 * q);
        self.set_coeffs(
            [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        );
    }

    #[inline]
    fn tick(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// Fourth-order Linkwitz-Riley section: two identical Butterworth biquads in series.
/// A low and high pass pair at one frequency sums to a flat all-pass.
#[derive(Clone, Copy, Default)]
struct Lr4([Biquad; 2]);

impl Lr4 {
    fn lowpass(fs: f64, f: f64) -> Self {
        let b = Biquad::lowpass(fs, f, std::f64::consts::FRAC_1_SQRT_2);
        Self([b, b])
    }

    fn highpass(fs: f64, f: f64) -> Self {
        let b = Biquad::highpass(fs, f, std::f64::consts::FRAC_1_SQRT_2);
        Self([b, b])
    }

    #[inline]
    fn tick(&mut self, x: f64) -> f64 {
        let y = self.0[0].tick(x);
        self.0[1].tick(y)
    }
}

/// Band splitter for one channel. The low band passes through an all-pass at the upper
/// crossover so that all three bands stay phase aligned and sum flat.
#[derive(Clone, Copy)]
struct Splitter {
    low: Lr4,
    rest: Lr4,
    mid: Lr4,
    high: Lr4,
    low_ap_lp: Lr4,
    low_ap_hp: Lr4,
}

impl Splitter {
    fn new(fs: f64) -> Self {
        Self {
            low: Lr4::lowpass(fs, LOW_MID_HZ),
            rest: Lr4::highpass(fs, LOW_MID_HZ),
            mid: Lr4::lowpass(fs, MID_HIGH_HZ),
            high: Lr4::highpass(fs, MID_HIGH_HZ),
            low_ap_lp: Lr4::lowpass(fs, MID_HIGH_HZ),
            low_ap_hp: Lr4::highpass(fs, MID_HIGH_HZ),
        }
    }

    #[inline]
    fn split(&mut self, x: f64) -> [f64; 3] {
        let low = self.low.tick(x);
        let rest = self.rest.tick(x);
        let low = self.low_ap_lp.tick(low) + self.low_ap_hp.tick(low);
        [low, self.mid.tick(rest), self.high.tick(rest)]
    }
}

/// The isolator's band split for one mono signal, for offline analysis such as the
/// loader's per-band waveform. Uses the same crossovers the EQ does, so the colours in
/// the waveform match what the EQ knobs change.
pub struct BandSplitter(Splitter);

impl BandSplitter {
    pub fn new(fs: f32) -> Self {
        Self(Splitter::new(fs as f64))
    }

    /// Split one sample into `[low, mid, high]`.
    #[inline]
    pub fn split(&mut self, x: f32) -> [f32; 3] {
        self.0.split(x as f64).map(|v| v as f32)
    }
}

/// DJ-style three-band isolator: each band from full kill up to +6 dB.
pub struct Isolator {
    split: [Splitter; 2],
    target_db: [f32; 3],
    kill: [bool; 3],
    gain: [f32; 3],
    /// 0 passes the input through untouched, 1 is fully equalised. Ramps between the two so
    /// leaving and returning to flat never clicks.
    wet: f32,
    smooth: f32,
}

impl Isolator {
    pub fn new(fs: f32) -> Self {
        Self {
            split: [Splitter::new(fs as f64); 2],
            target_db: [0.0; 3],
            kill: [false; 3],
            gain: [1.0; 3],
            wet: 0.0,
            smooth: smoothing_coeff(fs, SMOOTH_SECS),
        }
    }

    pub fn set_gain_db(&mut self, band: EqBand, db: f32) {
        self.target_db[band.index()] = db.clamp(EQ_MIN_DB, EQ_MAX_DB);
    }

    pub fn set_kill(&mut self, band: EqBand, kill: bool) {
        self.kill[band.index()] = kill;
    }

    fn targets(&self) -> [f32; 3] {
        std::array::from_fn(|i| {
            if self.kill[i] {
                0.0
            } else {
                db_to_gain(self.target_db[i])
            }
        })
    }

    /// Flat EQ is an exact pass-through: the crossovers keep running so their state is
    /// warm, but their all-pass phase shift never reaches the output.
    pub fn process(&mut self, buf: &mut [f32]) {
        let target = self.targets();
        let wet_target = if target == [1.0; 3] { 0.0 } else { 1.0 };
        for frame in buf.as_chunks_mut::<2>().0 {
            for (g, t) in self.gain.iter_mut().zip(target) {
                *g += (t - *g) * self.smooth;
            }
            self.wet += (wet_target - self.wet) * self.smooth;
            if (self.wet - wet_target).abs() < 1e-5 {
                self.wet = wet_target;
            }
            for (ch, s) in frame.iter_mut().enumerate() {
                let x = *s as f64;
                let [l, m, h] = self.split[ch].split(x);
                if self.wet > 0.0 {
                    let eq =
                        l * self.gain[0] as f64 + m * self.gain[1] as f64 + h * self.gain[2] as f64;
                    *s = (x + self.wet as f64 * (eq - x)) as f32;
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum FilterMode {
    Bypass,
    LowPass,
    HighPass,
}

/// One-knob filter: left of centre is a low-pass, right is a high-pass.
pub struct DjFilter {
    fs: f64,
    target: f32,
    value: f32,
    smooth: f32,
    mode: FilterMode,
    biquads: [Biquad; 2],
}

impl DjFilter {
    pub fn new(fs: f32) -> Self {
        Self {
            fs: fs as f64,
            target: 0.0,
            value: 0.0,
            // Smoothing runs once per sub-block, so scale the rate accordingly.
            smooth: smoothing_coeff(fs / FILTER_SUBBLOCK as f32, 0.02),
            mode: FilterMode::Bypass,
            biquads: [Biquad::default(); 2],
        }
    }

    /// Knob position from -1 (full low-pass) through 0 (off) to 1 (full high-pass).
    pub fn set(&mut self, value: f32) {
        self.target = value.clamp(-1.0, 1.0);
    }

    fn update_coeffs(&mut self) {
        let v = self.value;
        let mode = if v <= -FILTER_DEAD_ZONE {
            FilterMode::LowPass
        } else if v >= FILTER_DEAD_ZONE {
            FilterMode::HighPass
        } else {
            FilterMode::Bypass
        };
        if mode != self.mode {
            self.biquads.iter_mut().for_each(Biquad::reset);
            self.mode = mode;
        }
        let t = ((v.abs() - FILTER_DEAD_ZONE) / (1.0 - FILTER_DEAD_ZONE)).clamp(0.0, 1.0) as f64;
        let nyquist_guard = self.fs * 0.45;
        match mode {
            FilterMode::Bypass => {}
            FilterMode::LowPass => {
                let f = (FILTER_LP_MAX_HZ * (FILTER_LP_MIN_HZ / FILTER_LP_MAX_HZ).powf(t))
                    .min(nyquist_guard);
                self.biquads
                    .iter_mut()
                    .for_each(|b| b.set_lowpass(self.fs, f, FILTER_Q));
            }
            FilterMode::HighPass => {
                let f = (FILTER_HP_MIN_HZ * (FILTER_HP_MAX_HZ / FILTER_HP_MIN_HZ).powf(t))
                    .min(nyquist_guard);
                self.biquads
                    .iter_mut()
                    .for_each(|b| b.set_highpass(self.fs, f, FILTER_Q));
            }
        }
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        for block in buf.chunks_mut(FILTER_SUBBLOCK * 2) {
            self.value += (self.target - self.value) * self.smooth;
            if (self.value - self.target).abs() < 1e-4 {
                self.value = self.target;
            }
            self.update_coeffs();
            if self.mode == FilterMode::Bypass {
                continue;
            }
            for frame in block.as_chunks_mut::<2>().0 {
                for (ch, s) in frame.iter_mut().enumerate() {
                    *s = self.biquads[ch].tick(*s as f64) as f32;
                }
            }
        }
    }
}

/// Input gain, ±12 dB.
pub struct Trim {
    target: f32,
    gain: f32,
    smooth: f32,
}

impl Trim {
    pub fn new(fs: f32) -> Self {
        Self {
            target: 1.0,
            gain: 1.0,
            smooth: smoothing_coeff(fs, SMOOTH_SECS),
        }
    }

    pub fn set_db(&mut self, db: f32) {
        self.target = db_to_gain(db.clamp(-TRIM_RANGE_DB, TRIM_RANGE_DB));
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        if self.gain == self.target && self.gain == 1.0 {
            return;
        }
        for frame in buf.as_chunks_mut::<2>().0 {
            self.gain += (self.target - self.gain) * self.smooth;
            frame[0] *= self.gain;
            frame[1] *= self.gain;
        }
    }
}
