#![allow(dead_code)] // shared by several test binaries; each uses only part
//! Minimal WAV writer for fixtures, so tests don't depend on binary files in the repo.

use std::io::Write;
use std::path::Path;

pub enum Fmt {
    Pcm16,
    Float32,
}

/// Write interleaved samples (in -1..1) with the given channel count and rate.
pub fn write_wav(path: &Path, samples: &[f32], channels: u16, rate: u32, fmt: Fmt) {
    let (tag, bits): (u16, u16) = match fmt {
        Fmt::Pcm16 => (1, 16),
        Fmt::Float32 => (3, 32),
    };
    let block = channels * bits / 8;
    let data_len = samples.len() as u32 * bits as u32 / 8;
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(b"RIFF").unwrap();
    f.write_all(&(36 + data_len).to_le_bytes()).unwrap();
    f.write_all(b"WAVEfmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&tag.to_le_bytes()).unwrap();
    f.write_all(&channels.to_le_bytes()).unwrap();
    f.write_all(&rate.to_le_bytes()).unwrap();
    f.write_all(&(rate * block as u32).to_le_bytes()).unwrap();
    f.write_all(&block.to_le_bytes()).unwrap();
    f.write_all(&bits.to_le_bytes()).unwrap();
    f.write_all(b"data").unwrap();
    f.write_all(&data_len.to_le_bytes()).unwrap();
    for &s in samples {
        match fmt {
            Fmt::Pcm16 => f
                .write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
                .unwrap(),
            Fmt::Float32 => f.write_all(&s.to_le_bytes()).unwrap(),
        }
    }
}

pub fn sine(freq: f32, secs: f32, rate: u32, amp: f32) -> Vec<f32> {
    let n = (secs * rate as f32) as usize;
    (0..n)
        .map(|i| amp * (std::f32::consts::TAU * freq * i as f32 / rate as f32).sin())
        .collect()
}

pub fn stereo(mono: &[f32]) -> Vec<f32> {
    mono.iter().flat_map(|&s| [s, s]).collect()
}
