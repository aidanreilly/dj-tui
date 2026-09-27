//! Time stretching for key lock, so the tempo fader changes speed without changing pitch.
//!
//! WSOLA: cut overlapping grains out of the track, slide each one a little to line its
//! waveform up with what has already been written, and overlap-add them at the output rate.
//! The deck still moves its playhead at the tempo fader's rate, so everything that depends on
//! position, loops and cues among it, behaves exactly as it does without key lock. Only the
//! reading is different. Rubber Band would sound better on material this simple method
//! struggles with, but it is a C++ dependency for a job that a ±8 % fader barely stresses.

use crate::Track;

/// Grain length in frames at 48 kHz, scaled to the session rate. Around 40 ms, long enough
/// to hold a bass period and short enough that the alignment search stays cheap.
const GRAIN_AT_48K: usize = 2048;
/// How far a grain may slide to find its alignment, as a fraction of the grain.
const SEARCH_FRACTION: usize = 8;
/// The alignment search steps by this many frames. Finer than this buys nothing audible.
const SEARCH_STEP: usize = 8;

/// One deck's stretcher. Everything it needs is allocated up front.
pub struct Stretcher {
    /// Hann window over one grain.
    window: Vec<f32>,
    /// Overlap-add accumulator, interleaved stereo, one grain long.
    acc: Vec<f32>,
    /// Mono copy of the region the next grain has to line up with.
    target: Vec<f32>,
    /// Frames of `acc` already handed out since the last grain.
    read: usize,
    grain: usize,
    hop: usize,
    search: usize,
}

impl Stretcher {
    pub fn new(sample_rate: u32) -> Self {
        let grain = (GRAIN_AT_48K * sample_rate.max(1) as usize / 48_000).max(256) & !1;
        let hop = grain / 2;
        // Periodic Hann: two of these overlapped by half a grain sum to exactly one.
        let window = (0..grain)
            .map(|i| {
                let phase = std::f32::consts::TAU * i as f32 / grain as f32;
                0.5 - 0.5 * phase.cos()
            })
            .collect();
        Self {
            window,
            acc: vec![0.0; grain * 2],
            target: vec![0.0; hop],
            read: hop,
            grain,
            hop,
            search: grain / SEARCH_FRACTION,
        }
    }

    /// Forget what is in flight, for a new track or a switch back on.
    pub fn reset(&mut self) {
        self.acc.fill(0.0);
        self.target.fill(0.0);
        self.read = self.hop;
    }

    /// One output frame, reading around `pos`. The caller advances `pos` itself, so a seek or
    /// a loop wrap needs no telling: the next grain simply comes from where the playhead is.
    pub fn next_frame(&mut self, track: &Track, pos: f64) -> (f32, f32) {
        if self.read >= self.hop {
            self.fill(track, pos);
        }
        let i = self.read;
        self.read += 1;
        (self.acc[i * 2], self.acc[i * 2 + 1])
    }

    /// Slide the accumulator on by one hop and add the next grain into it.
    fn fill(&mut self, track: &Track, pos: f64) {
        let (grain, hop) = (self.grain, self.hop);
        self.acc.copy_within(hop * 2.., 0);
        self.acc[(grain - hop) * 2..].fill(0.0);
        for (i, slot) in self.target.iter_mut().enumerate() {
            *slot = (self.acc[i * 2] + self.acc[i * 2 + 1]) * 0.5;
        }

        let offset = self.best_offset(track, pos);
        for i in 0..grain {
            let (l, r) = track.frame_at(pos + offset + i as f64);
            let w = self.window[i];
            self.acc[i * 2] += l * w;
            self.acc[i * 2 + 1] += r * w;
        }
        self.read = 0;
    }

    /// Where to cut the next grain so its start matches the overlap already written.
    /// Normalising by the candidate's own energy stops a loud passage winning on volume alone.
    fn best_offset(&self, track: &Track, pos: f64) -> f64 {
        let radius = self.search as isize;
        let mut best = (f32::NEG_INFINITY, 0isize);
        let mut candidate = -radius;
        while candidate <= radius {
            let mut dot = 0.0f32;
            let mut energy = 1e-9f32;
            for (i, t) in self.target.iter().enumerate() {
                let (l, r) = track.frame_at(pos + (candidate + i as isize) as f64);
                let s = (l + r) * 0.5;
                dot += s * t;
                energy += s * s;
            }
            let score = dot / energy.sqrt();
            if score > best.0 {
                best = (score, candidate);
            }
            candidate += SEARCH_STEP as isize;
        }
        best.1 as f64
    }
}
