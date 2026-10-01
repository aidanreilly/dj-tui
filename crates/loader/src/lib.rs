//! Decoding and resampling, off the audio thread.
//!
//! Tracks are decoded completely into memory with symphonia, converted to stereo, resampled
//! to the session rate with rubato, and analysed for the overview waveform.

mod decode;
mod resample;
pub mod sidecar;

pub use decode::Tags;

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
    /// Peak `[low, mid, high]` per overview position, on the same scale as `waveform`.
    pub bands: Vec<[f32; 3]>,
    pub grid: Option<analysis::tempo::BeatGrid>,
    pub key: Option<analysis::key::Key>,
    /// Cues saved beside the file, in seconds.
    pub cues: sidecar::Cues,
    /// True when analysis and waveform came from the sidecar instead of being computed.
    pub from_sidecar: bool,
    /// Set when the sidecar couldn't be written; loading still succeeded.
    pub sidecar_note: Option<String>,
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

/// Decode `path` into a stereo track at `session_rate`, taking whatever analysis is saved
/// beside the file.
pub fn load_file(path: &Path, session_rate: u32) -> Result<LoadedTrack, LoadError> {
    load(path, session_rate, false)
}

/// The same for a batch run, which reads a file again when the saved analysis has no tempo
/// in it. Those are the ones a better detector can still rescue.
/// Read a file's tags without decoding it, for the browser's backfill pass.
pub fn read_tags(path: &Path) -> Result<Tags, LoadError> {
    decode::tags(path)
}

/// Write `tags` into the sidecar beside `path`, leaving the analysis and the cues alone.
/// Used by the backfill, which never opens the audio.
pub fn write_tags(path: &Path, tags: &Tags, checked_discogs: bool) -> std::io::Result<()> {
    let id = sidecar::fingerprint_file(path)?;
    let existing = sidecar::Sidecar::read(path).filter(|s| s.audio == id);
    let mut info = existing
        .as_ref()
        .and_then(|s| s.track.clone())
        .unwrap_or_default();
    info.title = tags.title.clone().or(info.title);
    info.artist = tags.artist.clone().or(info.artist);
    // A tag always wins over a lookup, and neither overwrites a genre already stored.
    info.genre = info.genre.or_else(|| tags.genre.clone());
    info.tags_read = true;
    info.discogs_checked = info.discogs_checked || checked_discogs;
    let file = sidecar::Sidecar {
        audio: id,
        analysis: existing.as_ref().and_then(|s| s.analysis.clone()),
        cues: existing.map(|s| s.cues).unwrap_or_default(),
        track: Some(info),
    };
    file.write(path)
}

pub fn analyse_file(path: &Path, session_rate: u32) -> Result<LoadedTrack, LoadError> {
    load(path, session_rate, true)
}

fn load(path: &Path, session_rate: u32, retry_empty: bool) -> Result<LoadedTrack, LoadError> {
    let decoded = decode::decode(path)?;
    let [l, r] = if decoded.sample_rate == session_rate {
        decoded.channels
    } else {
        resample::resample(decoded.channels, decoded.sample_rate, session_rate)?
    };
    let data: Vec<f32> = l.iter().zip(&r).flat_map(|(&a, &b)| [a, b]).collect();
    let track = Track::from_interleaved(data, session_rate);
    let id = sidecar::fingerprint_file(path).map_err(LoadError::Io)?;
    let saved = sidecar::Sidecar::read(path).filter(|s| s.audio == id);
    let cues = saved.as_ref().map(|s| s.cues.clone()).unwrap_or_default();
    let reusable = saved
        .and_then(|s| s.analysis)
        .filter(|a| a.waveform.len() == ENVELOPE_POINTS && a.bands.len() == ENVELOPE_POINTS)
        .filter(|a| a.grid.is_some() || !retry_empty);
    let from_sidecar = reusable.is_some();
    let stored = match reusable {
        Some(a) => a,
        None => sidecar::Stored {
            waveform: waveform_envelope(&track, ENVELOPE_POINTS),
            bands: band_envelope(&track, ENVELOPE_POINTS),
            grid: analysis::tempo::detect_tempo(&track, analysis::tempo::TempoRange::default()),
            key: analysis::key::detect_key(&track),
        },
    };
    let title = decoded.title.clone().unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    // The browser reads the tags and the length straight out of the sidecar, so they are
    // written even when the analysis came back from it.
    let saved_info = sidecar::Sidecar::read(path)
        .filter(|s| s.audio == id)
        .and_then(|s| s.track);
    let info = sidecar::TrackInfo {
        title: Some(title.clone()),
        artist: decoded.artist.clone(),
        duration_secs: Some(track.frames() as f64 / session_rate as f64),
        // The file's own tag wins. A stored genre survives only where the tag is silent,
        // which is what keeps a Discogs result from an earlier run.
        genre: decoded
            .genre
            .clone()
            .or_else(|| saved_info.as_ref().and_then(|t| t.genre.clone())),
        tags_read: true,
        discogs_checked: saved_info.as_ref().is_some_and(|t| t.discogs_checked),
    };
    let mut sidecar_note = None;
    if !from_sidecar || saved_info.as_ref() != Some(&info) {
        let file = sidecar::Sidecar {
            audio: id,
            analysis: Some(stored.clone()),
            cues: cues.clone(),
            track: Some(info),
        };
        if let Err(e) = file.write(path) {
            sidecar_note = Some(format!(
                "could not save {}: {e}",
                sidecar::sidecar_path(path).display()
            ));
        }
    }
    let sidecar::Stored {
        grid,
        key,
        waveform,
        bands,
    } = stored;
    Ok(LoadedTrack {
        track,
        title,
        artist: decoded.artist,
        waveform,
        bands,
        grid,
        key,
        cues,
        from_sidecar,
        sidecar_note,
    })
}

/// Peak level of the low, mid and high bands per overview position, split with the same
/// crossovers as the isolator EQ and normalised like [`waveform_envelope`].
pub fn band_envelope(track: &Track, points: usize) -> Vec<[f32; 3]> {
    let frames = track.frames();
    if frames == 0 || points == 0 {
        return vec![[0.0; 3]; points];
    }
    let mut split = engine::dsp::BandSplitter::new(track.sample_rate() as f32);
    let mut out = vec![[0.0f32; 3]; points];
    let mut full_peak = 0f32;
    for (j, slot) in out.iter_mut().enumerate() {
        let start = (j * frames / points).min(frames - 1);
        let end = ((j + 1) * frames / points).clamp(start + 1, frames);
        for i in start..end {
            let (l, r) = track.frame_at(i as f64);
            full_peak = full_peak.max(l.abs()).max(r.abs());
            let bands = split.split((l + r) * 0.5);
            for (acc, b) in slot.iter_mut().zip(bands) {
                *acc = acc.max(b.abs());
            }
        }
    }
    if full_peak > 0.0 {
        out.iter_mut().flatten().for_each(|v| *v /= full_peak);
    }
    out
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

enum Request {
    Load(DeckId, PathBuf),
    SaveCues(PathBuf, sidecar::Cues),
    /// Analyse a file for the library, keeping only what lands in the sidecar.
    Analyse(PathBuf),
    /// Read a file's tags into its sidecar. No audio is decoded.
    Tags(PathBuf),
    /// Ask Discogs about a file, by the artist and title its tags gave.
    Lookup(PathBuf, String, String),
}

/// One file the background analysis has finished with.
#[derive(Debug, Clone)]
pub struct Analysed {
    pub path: PathBuf,
    /// Why it could not be analysed, if it could not.
    pub error: Option<String>,
    /// The next request of the same kind would fail the same way, so the rest of the pass is
    /// not worth sending. A wrong Discogs token and a rate limit both say this.
    pub stop_pass: bool,
}

/// A background thread that loads files and saves cue changes, so the UI never waits on
/// the disk. Dropping it stops the thread.
pub struct Loader {
    requests: Sender<Request>,
    results: Receiver<LoadResult>,
    notes: Receiver<String>,
    analysed: Receiver<Analysed>,
}

impl Loader {
    pub fn spawn(session_rate: u32) -> Self {
        Self::spawn_with(session_rate, None)
    }

    /// `discogs` is the genre lookup, which is `None` unless the config switched it on and
    /// a token was in the environment.
    pub fn spawn_with(session_rate: u32, discogs: Option<discogs::Client>) -> Self {
        let (req_tx, req_rx) = channel::<Request>();
        let (res_tx, res_rx) = channel();
        let (note_tx, note_rx) = channel();
        let (analysed_tx, analysed_rx) = channel();
        std::thread::Builder::new()
            .name("dj-tui-loader".into())
            .spawn(move || {
                for req in req_rx {
                    match req {
                        Request::Load(deck, path) => {
                            let result = load_file(&path, session_rate);
                            if res_tx.send(LoadResult { deck, path, result }).is_err() {
                                break;
                            }
                        }
                        Request::Analyse(path) => {
                            // The decoded audio is dropped; the sidecar is the point.
                            let error = analyse_file(&path, session_rate)
                                .err()
                                .map(|e| e.to_string());
                            let done = Analysed {
                                path,
                                error,
                                stop_pass: false,
                            };
                            if analysed_tx.send(done).is_err() {
                                break;
                            }
                        }
                        Request::Tags(path) => {
                            let error = match read_tags(&path) {
                                Ok(tags) => {
                                    write_tags(&path, &tags, false).err().map(|e| e.to_string())
                                }
                                Err(e) => Some(e.to_string()),
                            };
                            let done = Analysed {
                                path,
                                error,
                                stop_pass: false,
                            };
                            if analysed_tx.send(done).is_err() {
                                break;
                            }
                        }
                        Request::Lookup(path, artist, title) => {
                            let (error, stop_pass) = match &discogs {
                                None => (Some("Discogs lookup is off".to_string()), true),
                                Some(client) => match client.genre(&artist, &title) {
                                    Ok(genre) => {
                                        // Written back whatever the answer, so a miss is not
                                        // asked about again on the next run.
                                        let tags = Tags {
                                            genre,
                                            ..Default::default()
                                        };
                                        let e = write_tags(&path, &tags, true)
                                            .err()
                                            .map(|e| e.to_string());
                                        (e, false)
                                    }
                                    Err(e) => (Some(e.to_string()), e.stops_the_pass()),
                                },
                            };
                            // Spaced so a batch stays inside the rate limit.
                            std::thread::sleep(discogs::MIN_REQUEST_GAP);
                            let done = Analysed {
                                path,
                                error,
                                stop_pass,
                            };
                            if analysed_tx.send(done).is_err() {
                                break;
                            }
                        }
                        Request::SaveCues(path, cues) => {
                            if let Err(e) = sidecar::save_cues(&path, &cues) {
                                let _ = note_tx.send(format!(
                                    "Could not save cues to {}: {e}",
                                    sidecar::sidecar_path(&path).display()
                                ));
                            }
                        }
                    }
                }
            })
            .expect("spawn loader thread");
        Self {
            requests: req_tx,
            results: res_rx,
            notes: note_rx,
            analysed: analysed_rx,
        }
    }

    pub fn request(&self, deck: DeckId, path: PathBuf) {
        // The thread only exits once `self` is gone, so this cannot fail while we exist.
        let _ = self.requests.send(Request::Load(deck, path));
    }

    /// Analyse `path` in the background, writing the results into its sidecar.
    pub fn analyse(&self, path: PathBuf) {
        let _ = self.requests.send(Request::Analyse(path));
    }

    /// Read `path`'s tags in the background and store them.
    pub fn read_tags_for(&self, path: PathBuf) {
        let _ = self.requests.send(Request::Tags(path));
    }

    /// Ask Discogs about `path` in the background.
    pub fn look_up(&self, path: PathBuf, artist: String, title: String) {
        let _ = self.requests.send(Request::Lookup(path, artist, title));
    }

    /// A file the background analysis has finished with, if one is ready.
    pub fn try_recv_analysed(&self) -> Option<Analysed> {
        self.analysed.try_recv().ok()
    }

    /// Write `cues` into the sidecar beside `path`, in the background.
    pub fn save_cues(&self, path: PathBuf, cues: sidecar::Cues) {
        let _ = self.requests.send(Request::SaveCues(path, cues));
    }

    pub fn try_recv(&self) -> Option<LoadResult> {
        self.results.try_recv().ok()
    }

    /// Problems from background saves, for the message line.
    pub fn try_recv_note(&self) -> Option<String> {
        self.notes.try_recv().ok()
    }
}
