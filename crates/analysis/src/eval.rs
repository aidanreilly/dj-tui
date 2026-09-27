//! Scoring the detectors against reference annotations (spec 3.5 and 6).
//!
//! The targets are 90 % tempo accuracy within ±2 %, octave errors flagged rather than failed,
//! and 65 % exact key accuracy. `examples/giantsteps.rs` runs this over a dataset.

use crate::key::{Key, Mode, PitchClass};

/// How far an estimate may sit from the annotation and still count, as a fraction.
const TEMPO_TOLERANCE: f64 = 0.02;
/// Ratios that count as an octave error: half, double and the three-against-two pair.
const OCTAVE_RATIOS: [f64; 4] = [0.5, 2.0, 1.5, 2.0 / 3.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempoVerdict {
    /// Within the tolerance of the annotation.
    Exact,
    /// Within the tolerance of a half, double, three-halves or two-thirds of it.
    Octave,
    Wrong,
}

/// Score one tempo estimate against its annotation.
pub fn tempo_verdict(estimate: f64, truth: f64) -> TempoVerdict {
    if !estimate.is_finite() || !truth.is_finite() || estimate <= 0.0 || truth <= 0.0 {
        return TempoVerdict::Wrong;
    }
    if (estimate - truth).abs() <= truth * TEMPO_TOLERANCE {
        return TempoVerdict::Exact;
    }
    for ratio in OCTAVE_RATIOS {
        let folded = truth * ratio;
        if (estimate - folded).abs() <= folded * TEMPO_TOLERANCE {
            return TempoVerdict::Octave;
        }
    }
    TempoVerdict::Wrong
}

/// MIREX key weights: a fifth away, the relative and the parallel are each worth part marks,
/// because a mix in any of them is closer to working than one in an unrelated key.
pub fn key_score(estimate: Key, truth: Key) -> f64 {
    if estimate == truth {
        return 1.0;
    }
    let distance = (estimate.tonic.0 as i32 - truth.tonic.0 as i32).rem_euclid(12);
    if estimate.mode == truth.mode && (distance == 7 || distance == 5) {
        return 0.5;
    }
    let relative = match truth.mode {
        // A minor is the relative of C major, three semitones below it.
        Mode::Major => (truth.tonic.0 as i32 + 9).rem_euclid(12),
        Mode::Minor => (truth.tonic.0 as i32 + 3).rem_euclid(12),
    };
    if estimate.mode != truth.mode && estimate.tonic.0 as i32 == relative {
        return 0.3;
    }
    if estimate.mode != truth.mode && distance == 0 {
        return 0.2;
    }
    0.0
}

/// Note names as the annotations write them, sharps and flats both.
const NOTES: [(&str, u8); 21] = [
    ("c", 0),
    ("c#", 1),
    ("db", 1),
    ("d", 2),
    ("d#", 3),
    ("eb", 3),
    ("e", 4),
    ("fb", 4),
    ("f", 5),
    ("f#", 6),
    ("gb", 6),
    ("g", 7),
    ("g#", 8),
    ("ab", 8),
    ("a", 9),
    ("a#", 10),
    ("bb", 10),
    ("b", 11),
    ("cb", 11),
    ("e#", 5),
    ("b#", 0),
];

/// Read one key annotation: `A minor`, `F# major`, a bare tonic, or a Camelot code.
pub fn parse_key_annotation(text: &str) -> Option<Key> {
    let text = text.trim().trim_end_matches(['.', ';']);
    if text.is_empty() {
        return None;
    }
    if let Some(key) = Key::from_camelot(text) {
        return Some(key);
    }
    let lower = text.to_ascii_lowercase().replace(['\t', '-', '_'], " ");
    let mut parts = lower.split_whitespace();
    let tonic = parts.next()?;
    let (name, pitch) = NOTES.iter().find(|(name, _)| *name == tonic)?;
    let _ = name;
    let mode = match parts.next() {
        None => Mode::Major,
        Some(word) if word.starts_with("maj") => Mode::Major,
        Some(word) if word.starts_with("min") || word == "m" => Mode::Minor,
        Some(_) => return None,
    };
    Some(Key {
        tonic: PitchClass(*pitch),
        mode,
    })
}

/// Running totals over a dataset.
#[derive(Debug, Clone, Default)]
pub struct Accuracy {
    pub tempo_tracks: usize,
    pub tempo_exact: usize,
    pub tempo_octave: usize,
    pub key_tracks: usize,
    pub key_exact: usize,
    /// Sum of the MIREX weights, which the weighted score averages.
    pub key_weight: f64,
}

impl Accuracy {
    pub fn add_tempo(&mut self, verdict: TempoVerdict) {
        self.tempo_tracks += 1;
        match verdict {
            TempoVerdict::Exact => self.tempo_exact += 1,
            TempoVerdict::Octave => self.tempo_octave += 1,
            TempoVerdict::Wrong => {}
        }
    }

    pub fn add_key(&mut self, score: f64) {
        self.key_tracks += 1;
        if score >= 1.0 {
            self.key_exact += 1;
        }
        self.key_weight += score;
    }

    /// Share of tracks whose tempo landed within the tolerance.
    pub fn tempo_exact(&self) -> f64 {
        ratio(self.tempo_exact as f64, self.tempo_tracks)
    }

    /// Share that landed on the tempo or one of its octaves.
    pub fn tempo_within_octave(&self) -> f64 {
        ratio(
            (self.tempo_exact + self.tempo_octave) as f64,
            self.tempo_tracks,
        )
    }

    pub fn key_exact(&self) -> f64 {
        ratio(self.key_exact as f64, self.key_tracks)
    }

    pub fn key_weighted(&self) -> f64 {
        ratio(self.key_weight, self.key_tracks)
    }
}

fn ratio(count: f64, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        count / total as f64
    }
}
