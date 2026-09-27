//! Key detection: a whole-track chromagram correlated against major and minor profiles.

use engine::Track;
use realfft::RealFftPlanner;

const WIN: usize = 16_384;
const HOP: usize = 8_192;
const MIN_HZ: f32 = 50.0;
const MAX_HZ: f32 = 2_000.0;
const C0_HZ: f32 = 16.351_6;

/// Krumhansl-Kessler probe-tone profiles, C first.
const MAJOR: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

const NAMES: [&str; 12] = [
    "C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B",
];

/// Pitch class, 0 = C.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PitchClass(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Major,
    Minor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub tonic: PitchClass,
    pub mode: Mode,
}

impl Key {
    /// Camelot wheel code: 8A is A minor, 8B is C major; neighbours differ by a fifth.
    pub fn camelot(&self) -> String {
        let (offset, letter) = match self.mode {
            Mode::Minor => (5, 'A'),
            Mode::Major => (8, 'B'),
        };
        let n = (self.tonic.0 as u32 * 7 + offset - 1) % 12 + 1;
        format!("{n}{letter}")
    }

    /// Parse a Camelot code such as `8A` or `11B`.
    pub fn from_camelot(code: &str) -> Option<Key> {
        let (num, letter) = code.trim().split_at(code.trim().len().checked_sub(1)?);
        let n: u32 = num.parse().ok().filter(|n| (1..=12).contains(n))?;
        let (offset, mode) = match letter {
            "A" | "a" => (5, Mode::Minor),
            "B" | "b" => (8, Mode::Major),
            _ => return None,
        };
        // Invert `n = (7·pc + offset - 1) mod 12 + 1`; 7 is its own inverse mod 12.
        let pc = ((n + 12 * 2 - offset) * 7) % 12;
        Some(Key {
            tonic: PitchClass(pc as u8),
            mode,
        })
    }

    pub fn name(&self) -> String {
        let mode = match self.mode {
            Mode::Major => "major",
            Mode::Minor => "minor",
        };
        format!("{} {mode}", NAMES[self.tonic.0 as usize % 12])
    }
}

fn chroma(track: &Track) -> Option<[f32; 12]> {
    let n = track.frames();
    if n < WIN {
        return None;
    }
    let fs = track.sample_rate() as f32;
    let fft = RealFftPlanner::<f32>::new().plan_fft_forward(WIN);
    let mut input = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let window: Vec<f32> = (0..WIN)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / WIN as f32).cos())
        .collect();
    let bin_pc: Vec<Option<usize>> = (0..spectrum.len())
        .map(|b| {
            let f = b as f32 * fs / WIN as f32;
            (MIN_HZ..=MAX_HZ)
                .contains(&f)
                .then(|| (12.0 * (f / C0_HZ).log2()).round() as i64)
                .map(|p| p.rem_euclid(12) as usize)
        })
        .collect();
    let mut acc = [0f32; 12];
    for start in (0..=n - WIN).step_by(HOP) {
        for (i, (d, w)) in input.iter_mut().zip(&window).enumerate() {
            let (l, r) = track.frame_at((start + i) as f64);
            *d = (l + r) * 0.5 * w;
        }
        fft.process(&mut input, &mut spectrum)
            .expect("fft sizes match");
        for (c, pc) in spectrum.iter().zip(&bin_pc) {
            if let Some(pc) = pc {
                acc[*pc] += c.norm_sqr().sqrt();
            }
        }
    }
    let total: f32 = acc.iter().sum();
    (total > 1e-3).then_some(acc)
}

fn correlation(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let (ma, mb) = (a.iter().sum::<f32>() / 12.0, b.iter().sum::<f32>() / 12.0);
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        num += (a[i] - ma) * (b[i] - mb);
        da += (a[i] - ma).powi(2);
        db += (b[i] - mb).powi(2);
    }
    num / (da * db).sqrt().max(1e-12)
}

pub fn detect_key(track: &Track) -> Option<Key> {
    let c = chroma(track)?;
    let mut best = (
        f32::MIN,
        Key {
            tonic: PitchClass(0),
            mode: Mode::Major,
        },
    );
    for tonic in 0..12u8 {
        for (mode, profile) in [(Mode::Major, &MAJOR), (Mode::Minor, &MINOR)] {
            let rotated: [f32; 12] =
                std::array::from_fn(|i| profile[(i + 12 - tonic as usize) % 12]);
            let r = correlation(&c, &rotated);
            if r > best.0 {
                best = (
                    r,
                    Key {
                        tonic: PitchClass(tonic),
                        mode,
                    },
                );
            }
        }
    }
    Some(best.1)
}
