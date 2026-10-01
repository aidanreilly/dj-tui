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
/// A decodable WAV with a LIST/INFO chunk, for the tests that go all the way through a load.
/// Symphonia maps IART, INAM and IGNR onto Artist, TrackTitle and Genre.
pub fn write_tagged_wav(
    path: &Path,
    samples: &[f32],
    rate: u32,
    artist: &str,
    title: &str,
    genre: &str,
) {
    write_wav(path, samples, 2, rate, Fmt::Float32);
    let mut info: Vec<u8> = b"INFO".to_vec();
    for (id, value) in [(b"IART", artist), (b"INAM", title), (b"IGNR", genre)] {
        let mut text = value.as_bytes().to_vec();
        text.push(0);
        if text.len() % 2 == 1 {
            text.push(0);
        }
        info.extend_from_slice(id);
        info.extend_from_slice(&(text.len() as u32).to_le_bytes());
        info.extend_from_slice(&text);
    }
    let file = std::fs::read(path).expect("the wav just written");
    // The LIST chunk goes ahead of `data`. A reader streams the audio from the data chunk
    // onwards and never looks past it, so a LIST after it is never seen.
    let at = file
        .windows(4)
        .position(|w| w == b"data")
        .expect("a data chunk");
    let mut out = Vec::with_capacity(file.len() + info.len() + 8);
    out.extend_from_slice(&file[..at]);
    out.extend_from_slice(b"LIST");
    out.extend_from_slice(&(info.len() as u32).to_le_bytes());
    out.extend_from_slice(&info);
    out.extend_from_slice(&file[at..]);
    // The RIFF size covers everything after the first eight bytes.
    let riff = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&riff.to_le_bytes());
    std::fs::write(path, out).expect("rewrite the wav with its tags");
}
