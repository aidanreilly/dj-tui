//! Per-track data kept beside the audio file as `<file name>.dj-tui.json`.
//!
//! Analysis (BPM, beat grid, key), the overview waveform and cues travel with the music,
//! so they survive moving a library between machines. A fingerprint of the audio file
//! detects when the file changed, which invalidates the analysis and cues.

use analysis::{key::Key, tempo::BeatGrid};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const SIDECAR_VERSION: u32 = 1;
const SUFFIX: &str = ".dj-tui.json";
const FORMAT: &str = "dj-tui track data";
/// Bytes hashed from the start of the audio file.
const FINGERPRINT_BYTES: usize = 1 << 20;

pub fn sidecar_path(audio: &Path) -> PathBuf {
    let mut name = audio.file_name().unwrap_or_default().to_os_string();
    name.push(SUFFIX);
    audio.with_file_name(name)
}

/// Identifies one version of an audio file: its size plus a hash of its first megabyte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioId {
    pub file_size: u64,
    #[serde(with = "hex")]
    pub fingerprint: u64,
}

mod hex {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{v:016x}"))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let s = String::deserialize(d)?;
        u64::from_str_radix(&s, 16).map_err(serde::de::Error::custom)
    }
}

/// FNV-1a, 64 bit. Chosen because its output never changes between Rust releases.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

pub fn fingerprint_file(path: &Path) -> std::io::Result<AudioId> {
    let file = std::fs::File::open(path)?;
    let file_size = file.metadata()?.len();
    let mut head = Vec::with_capacity(FINGERPRINT_BYTES.min(file_size as usize));
    file.take(FINGERPRINT_BYTES as u64).read_to_end(&mut head)?;
    Ok(AudioId {
        file_size,
        fingerprint: fnv1a(&head),
    })
}

/// Cue and loop positions in seconds of track time.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cues {
    pub main_cue_secs: Option<f64>,
    pub hot_cues: [Option<f64>; 8],
    /// The loop left running on the deck, as (start, end).
    pub loop_secs: Option<(f64, f64)>,
}

#[derive(Serialize, Deserialize)]
struct HotCueJson {
    /// One-based pad number, as printed on the pads.
    pad: usize,
    secs: f64,
}

#[derive(Serialize, Deserialize, Default)]
struct CuesJson {
    main_cue_secs: Option<f64>,
    hot_cues: Vec<HotCueJson>,
    /// Absent in sidecars written before loops were saved.
    #[serde(default)]
    loop_secs: Option<[f64; 2]>,
}

impl From<&Cues> for CuesJson {
    fn from(c: &Cues) -> Self {
        Self {
            main_cue_secs: c.main_cue_secs,
            hot_cues: c
                .hot_cues
                .iter()
                .enumerate()
                .filter_map(|(i, s)| s.map(|secs| HotCueJson { pad: i + 1, secs }))
                .collect(),
            loop_secs: c.loop_secs.map(|(start, end)| [start, end]),
        }
    }
}

impl From<&CuesJson> for Cues {
    fn from(j: &CuesJson) -> Self {
        let mut hot_cues = [None; 8];
        for h in &j.hot_cues {
            if let Some(slot) = h.pad.checked_sub(1).and_then(|i| hot_cues.get_mut(i)) {
                *slot = Some(h.secs);
            }
        }
        Self {
            main_cue_secs: j.main_cue_secs,
            hot_cues,
            // A loop whose end is not past its start would trap the playhead, so drop it.
            loop_secs: j
                .loop_secs
                .map(|[start, end]| (start, end))
                .filter(|(start, end)| end > start),
        }
    }
}

/// What a track list shows without opening the audio: the tags and the length.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct TrackInfo {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_secs: Option<f64>,
    /// From the file's own tags, or from a matched Discogs release. Never from the audio.
    #[serde(default)]
    pub genre: Option<String>,
    /// The file's tags have been read. Separates "no genre tag" from "never looked", which
    /// both store `genre: None`.
    #[serde(default)]
    pub tags_read: bool,
    /// A Discogs lookup has been run and is not worth running again.
    #[serde(default)]
    pub discogs_checked: bool,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct AnalysisJson {
    pub bpm: Option<f64>,
    pub first_beat_secs: Option<f64>,
    /// Camelot code, e.g. "8A".
    pub key: Option<String>,
    /// Readable name, e.g. "A minor". Informational; `key` is what gets read back.
    pub key_name: Option<String>,
}

/// Waveform quantised to keep the file small: ranges as -127..127, bands as 0..255.
#[derive(Serialize, Deserialize, Default)]
struct WaveformJson {
    points: usize,
    ranges: Vec<[i8; 2]>,
    bands: Vec<[u8; 3]>,
}

#[derive(Serialize, Deserialize)]
struct FileJson {
    format: String,
    version: u32,
    audio: AudioId,
    #[serde(default)]
    analysis: Option<AnalysisJson>,
    #[serde(default)]
    cues: CuesJson,
    #[serde(default)]
    waveform: Option<WaveformJson>,
    /// Absent in sidecars written before the browser needed these.
    #[serde(default)]
    track: Option<TrackInfo>,
}

/// Analysis results as stored and restored.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stored {
    pub grid: Option<BeatGrid>,
    pub key: Option<Key>,
    pub waveform: Vec<[f32; 2]>,
    pub bands: Vec<[f32; 3]>,
}

/// What the sidecar holds for one audio file.
pub struct Sidecar {
    pub audio: AudioId,
    /// `None` until the track has been analysed.
    pub analysis: Option<Stored>,
    pub cues: Cues,
    /// Tags and length, for a track list that has not opened the file.
    pub track: Option<TrackInfo>,
}

impl Sidecar {
    /// Read the sidecar for `audio`. Missing, unreadable, corrupt or other-version files
    /// all come back as `None`, and are rewritten on the next save.
    pub fn read(audio: &Path) -> Option<Sidecar> {
        let text = std::fs::read_to_string(sidecar_path(audio)).ok()?;
        let j: FileJson = serde_json::from_str(&text).ok()?;
        if j.format != FORMAT || j.version != SIDECAR_VERSION {
            return None;
        }
        let analysis = match (j.analysis, j.waveform) {
            (Some(a), Some(w)) => Some(Stored {
                grid: a
                    .bpm
                    .zip(a.first_beat_secs)
                    .map(|(bpm, first_beat_secs)| BeatGrid {
                        bpm,
                        first_beat_secs,
                    }),
                key: a.key.as_deref().and_then(Key::from_camelot),
                waveform: w
                    .ranges
                    .iter()
                    .map(|r| [r[0] as f32 / 127.0, r[1] as f32 / 127.0])
                    .collect(),
                bands: w
                    .bands
                    .iter()
                    .map(|b| b.map(|v| v as f32 / 255.0))
                    .collect(),
            }),
            _ => None,
        };
        Some(Sidecar {
            audio: j.audio,
            analysis,
            cues: (&j.cues).into(),
            track: j.track,
        })
    }

    /// Write atomically: a temporary file in the same directory, then a rename.
    pub fn write(&self, audio: &Path) -> std::io::Result<()> {
        let (analysis, waveform) = match &self.analysis {
            Some(s) => (
                Some(AnalysisJson {
                    bpm: s.grid.map(|g| g.bpm),
                    first_beat_secs: s.grid.map(|g| g.first_beat_secs),
                    key: s.key.map(|k| k.camelot()),
                    key_name: s.key.map(|k| k.name()),
                }),
                Some(WaveformJson {
                    points: s.waveform.len(),
                    ranges: s
                        .waveform
                        .iter()
                        .map(|r| r.map(|v| (v.clamp(-1.0, 1.0) * 127.0).round() as i8))
                        .collect(),
                    bands: s
                        .bands
                        .iter()
                        .map(|b| b.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8))
                        .collect(),
                }),
            ),
            None => (None, None),
        };
        let j = FileJson {
            format: FORMAT.into(),
            version: SIDECAR_VERSION,
            audio: self.audio,
            analysis,
            cues: (&self.cues).into(),
            waveform,
            track: self.track.clone(),
        };
        let path = sidecar_path(audio);
        let mut tmp = path.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        std::fs::write(
            &tmp,
            serde_json::to_string(&j).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(&tmp, &path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }
}

/// Update only the cues in `audio`'s sidecar, keeping any valid analysis.
pub fn save_cues(audio: &Path, cues: &Cues) -> std::io::Result<()> {
    let id = fingerprint_file(audio)?;
    let analysis = Sidecar::read(audio)
        .filter(|s| s.audio == id)
        .and_then(|s| s.analysis);
    let track = Sidecar::read(audio)
        .filter(|s| s.audio == id)
        .and_then(|s| s.track);
    Sidecar {
        audio: id,
        analysis,
        cues: cues.clone(),
        track,
    }
    .write(audio)
}
