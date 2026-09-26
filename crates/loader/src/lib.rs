//! Decoding and resampling, off the audio thread.
//!
//! Tracks are decoded completely into memory with symphonia, converted to stereo, resampled
//! to the session rate with rubato, and analysed for the overview waveform.

mod decode;
mod resample;

use engine::{DeckId, Track};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

/// Resolution of the stored overview waveform. The TUI downsamples to the panel width.
pub const ENVELOPE_POINTS: usize = 2048;

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    /// Container or codec not recognised.
    Unsupported(String),
    /// Recognised, but decoding failed part way.
    Decode(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "cannot read file: {e}"),
            LoadError::Unsupported(e) => write!(f, "unsupported format: {e}"),
            LoadError::Decode(e) => write!(f, "decode failed: {e}"),
        }
    }
}

impl std::error::Error for LoadError {}

pub struct LoadedTrack {
    pub track: Track,
    pub title: String,
    pub artist: Option<String>,
    /// Signed `[minimum, maximum]` sample range for each overview position.
    pub waveform: Vec<[f32; 2]>,
}

impl std::fmt::Debug for LoadedTrack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedTrack")
            .field("track", &self.track)
            .field("title", &self.title)
            .field("artist", &self.artist)
            .finish_non_exhaustive()
    }
}

/// Decode `path` into a stereo track at `session_rate`.
pub fn load_file(path: &Path, session_rate: u32) -> Result<LoadedTrack, LoadError> {
    let decoded = decode::decode(path)?;
    let [l, r] = if decoded.sample_rate == session_rate {
        decoded.channels
    } else {
        resample::resample(decoded.channels, decoded.sample_rate, session_rate)?
    };
    let data: Vec<f32> = l.iter().zip(&r).flat_map(|(&a, &b)| [a, b]).collect();
    let track = Track::from_interleaved(data, session_rate);
    let waveform = waveform_envelope(&track, ENVELOPE_POINTS);
    let title = decoded.title.unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    Ok(LoadedTrack {
        track,
        title,
        artist: decoded.artist,
        waveform,
    })
}

/// Whole-track signed amplitude ranges, normalized so the loudest absolute sample is 1.
/// Each point stores `[minimum, maximum]` across both channels for its time bucket.
pub fn waveform_envelope(track: &Track, points: usize) -> Vec<[f32; 2]> {
    let frames = track.frames();
    if frames == 0 || points == 0 {
        return vec![[0.0, 0.0]; points];
    }

    let mut waveform: Vec<[f32; 2]> = (0..points)
        .map(|j| {
            let start = (j * frames / points).min(frames - 1);
            let end = ((j + 1) * frames / points).clamp(start + 1, frames);
            let mut min = f32::INFINITY;
            let mut max = f32::NEG_INFINITY;
            for i in start..end {
                let (l, r) = track.frame_at(i as f64);
                min = min.min(l).min(r);
                max = max.max(l).max(r);
            }
            [min, max]
        })
        .collect();

    let peak = waveform
        .iter()
        .flat_map(|[min, max]| [min.abs(), max.abs()])
        .fold(0.0, f32::max);
    if peak > 0.0 {
        waveform.iter_mut().for_each(|range| {
            range[0] /= peak;
            range[1] /= peak;
        });
    }
    waveform
}

/// Whole-track peak envelope with `points` values, normalised so the loudest is 1.
/// Retained for callers that need a magnitude-only envelope.
pub fn peak_envelope(track: &Track, points: usize) -> Vec<f32> {
    let frames = track.frames();
    if frames == 0 || points == 0 {
        return vec![0.0; points];
    }
    let mut env: Vec<f32> = (0..points)
        .map(|j| {
            let start = (j * frames / points).min(frames - 1);
            let end = ((j + 1) * frames / points).clamp(start + 1, frames);
            (start..end)
                .map(|i| {
                    let (l, r) = track.frame_at(i as f64);
                    l.abs().max(r.abs())
                })
                .fold(0.0, f32::max)
        })
        .collect();
    let max = env.iter().copied().fold(0.0, f32::max);
    if max > 0.0 {
        env.iter_mut().for_each(|v| *v /= max);
    }
    env
}

pub struct LoadResult {
    pub deck: DeckId,
    pub path: PathBuf,
    pub result: Result<LoadedTrack, LoadError>,
}

/// A background thread that loads files on request. Dropping it stops the thread.
pub struct Loader {
    requests: Sender<(DeckId, PathBuf)>,
    results: Receiver<LoadResult>,
}

impl Loader {
    pub fn spawn(session_rate: u32) -> Self {
        let (req_tx, req_rx) = channel::<(DeckId, PathBuf)>();
        let (res_tx, res_rx) = channel();
        std::thread::Builder::new()
            .name("dj-tui-loader".into())
            .spawn(move || {
                for (deck, path) in req_rx {
                    let result = load_file(&path, session_rate);
                    if res_tx.send(LoadResult { deck, path, result }).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn loader thread");
        Self {
            requests: req_tx,
            results: res_rx,
        }
    }

    pub fn request(&self, deck: DeckId, path: PathBuf) {
        // The thread only exits once `self` is gone, so this cannot fail while we exist.
        let _ = self.requests.send((deck, path));
    }

    pub fn try_recv(&self) -> Option<LoadResult> {
        self.results.try_recv().ok()
    }
}
