//! Builds the view structs the TUI renders from an engine snapshot plus per-deck metadata.

use engine::{DeckId, Snapshot, HOT_CUES};
use tui::{DeckView, MixerView, ScreenView};

/// Per-deck information that lives outside the engine: metadata and analysis results.
#[derive(Debug, Default, Clone)]
pub struct DeckMeta {
    pub title: Option<String>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub loading: bool,
    pub waveform: Vec<[f32; 2]>,
    pub bands: Vec<[f32; 3]>,
    pub grid: Option<analysis::tempo::BeatGrid>,
}

fn deck_view(snap: &Snapshot, rate: f64, id: DeckId, focused: DeckId, meta: &DeckMeta) -> DeckView {
    let d = &snap.decks[id.index()];
    let loaded = d.track_frames > 0;
    let mut hot_cues = [false; HOT_CUES];
    for (slot, cue) in hot_cues.iter_mut().zip(&d.hot_cues) {
        *slot = cue.is_some();
    }
    DeckView {
        id,
        focused: id == focused,
        title: if loaded { meta.title.clone() } else { None },
        // Shown at the current tempo fader setting.
        bpm: meta.bpm.map(|b| b * d.rate),
        key: meta.key.clone(),
        position_secs: d.position / rate,
        duration_secs: d.track_frames as f64 / rate,
        loading: meta.loading,
        playing: d.playing,
        hot_cues,
        waveform: if loaded {
            meta.waveform.clone()
        } else {
            Vec::new()
        },
        bands: if loaded {
            meta.bands.clone()
        } else {
            Vec::new()
        },
        waveform_mode: Default::default(),
        beat: meta
            .grid
            .filter(|_| loaded)
            .map(|g| g.bar_and_beat(d.position / rate)),
        cue_secs: loaded.then(|| d.cue_point / rate),
        hot_cue_secs: d.hot_cues.map(|c| c.filter(|_| loaded).map(|f| f / rate)),
        loop_secs: d
            .loop_span
            .filter(|_| loaded)
            .map(|(start, end)| (start / rate, end / rate)),
        loop_in_secs: None,
        quantize: false,
        end_warning: false,
    }
}

/// `sample_rate` is the session rate; every loaded track has been resampled to it.
pub fn screen_view(
    snap: &Snapshot,
    sample_rate: u32,
    focused: DeckId,
    metas: &[DeckMeta; 2],
    status: String,
) -> ScreenView {
    let rate = sample_rate as f64;
    ScreenView {
        decks: [
            deck_view(snap, rate, DeckId::A, focused, &metas[0]),
            deck_view(snap, rate, DeckId::B, focused, &metas[1]),
        ],
        mixer: MixerView {
            crossfader: snap.crossfader,
            faders: snap.faders,
            headphone_cue: snap.headphone_cue,
            ..Default::default()
        },
        status,
        message: String::new(),
        phase: match (&metas[0].grid, &metas[1].grid) {
            (Some(a), Some(b))
                if snap.decks[0].track_frames > 0 && snap.decks[1].track_frames > 0 =>
            {
                Some(phase_offset(
                    a.phase(snap.decks[0].position / rate),
                    b.phase(snap.decks[1].position / rate),
                ))
            }
            _ => None,
        },
    }
}

/// Seconds of flashing per on/off cycle of the end-of-track warning.
const WARNING_FLASH_SECS: f64 = 1.0;

/// Whether the end-of-track warning is lit right now: playing, inside the last
/// `threshold_secs`, and in the on half of the flash. A threshold of 0 disables it.
pub fn end_warning(
    remaining_secs: f64,
    playing: bool,
    threshold_secs: u32,
    elapsed_secs: f64,
) -> bool {
    playing
        && threshold_secs > 0
        && remaining_secs < threshold_secs as f64
        && (elapsed_secs / WARNING_FLASH_SECS).fract() < 0.5
}

/// Deck B's beat phase minus deck A's, wrapped into -0.5..0.5 beats.
pub fn phase_offset(a: f64, b: f64) -> f64 {
    (b - a + 0.5).rem_euclid(1.0) - 0.5
}
