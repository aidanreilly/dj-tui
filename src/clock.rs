//! Stand-in for an audio backend: advances the engine by wall-clock time and discards output.
//! Replaced by the JACK backend in M1; kept afterwards for `--no-audio` runs.

use engine::{Engine, MAX_BLOCK_FRAMES};
use std::time::Duration;

pub struct NullClock {
    sample_rate: f64,
    carry: f64,
    master: Vec<f32>,
    cue: Vec<f32>,
}

impl NullClock {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate as f64,
            carry: 0.0,
            master: vec![0.0; MAX_BLOCK_FRAMES * 2],
            cue: vec![0.0; MAX_BLOCK_FRAMES * 2],
        }
    }

    pub fn advance(&mut self, engine: &mut Engine, elapsed: Duration) {
        let exact = elapsed.as_secs_f64() * self.sample_rate + self.carry;
        let mut frames = exact.floor() as usize;
        self.carry = exact - frames as f64;
        while frames > 0 {
            let n = frames.min(MAX_BLOCK_FRAMES);
            engine.process(&mut self.master[..n * 2], &mut self.cue[..n * 2]);
            frames -= n;
        }
    }
}
