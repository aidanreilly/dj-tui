//! One RBJ second-order section, direct form 1.

use std::f32::consts::TAU;

/// Butterworth Q, the flattest response without a resonant peak.
pub const Q: f32 = 0.707;

#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

/// Angular frequency and the RBJ alpha term, with the corner held below Nyquist.
/// Low sample rate sources (telephone-quality MP3) would otherwise ask for a corner
/// the filter cannot express.
fn corner(cutoff_hz: f32, sample_rate: f32) -> (f32, f32) {
    let nyquist = sample_rate / 2.0;
    let f0 = cutoff_hz.clamp(1.0, nyquist * 0.98);
    let w0 = TAU * f0 / sample_rate;
    (w0, w0.sin() / (2.0 * Q))
}

impl Biquad {
    pub fn low_pass(cutoff_hz: f32, sample_rate: f32) -> Self {
        let (w0, alpha) = corner(cutoff_hz, sample_rate);
        let cos = w0.cos();
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 - cos) / 2.0) / a0,
            b1: (1.0 - cos) / a0,
            b2: ((1.0 - cos) / 2.0) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha) / a0,
            ..Default::default()
        }
    }

    pub fn high_pass(cutoff_hz: f32, sample_rate: f32) -> Self {
        let (w0, alpha) = corner(cutoff_hz, sample_rate);
        let cos = w0.cos();
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 + cos) / 2.0) / a0,
            b1: (-(1.0 + cos)) / a0,
            b2: ((1.0 + cos) / 2.0) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha) / a0,
            ..Default::default()
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}
