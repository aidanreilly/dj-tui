//! Builds the view structs the TUI renders from an engine snapshot plus per-deck metadata.

use engine::{DeckId, Snapshot, HOT_CUES};
use tui::{DeckView, MixerView, ScreenView};

/// Per-deck information that lives outside the engine: metadata and analysis results.
#[derive(Debug, Default, Clone)]
pub struct DeckMeta {
    pub title: Option<String>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub envelope: Vec<f32>,
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
        bpm: meta.bpm,
        key: meta.key.clone(),
        position_secs: d.position / rate,
        duration_secs: d.track_frames as f64 / rate,
        playing: d.playing,
        hot_cues,
        envelope: if loaded { meta.envelope.clone() } else { Vec::new() },
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
        mixer: MixerView { crossfader: snap.crossfader, faders: snap.faders, headphone_cue: snap.headphone_cue },
        status,
    }
}
