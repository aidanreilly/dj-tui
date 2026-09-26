//! Copies engine state into the view structs the TUI renders.

use engine::{DeckId, Engine, HOT_CUES};
use tui::{DeckView, MixerView, ScreenView};

/// Per-deck information that lives outside the engine: metadata and analysis results.
#[derive(Debug, Default, Clone)]
pub struct DeckMeta {
    pub title: Option<String>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub envelope: Vec<f32>,
}

fn deck_view(e: &Engine, id: DeckId, focused: DeckId, meta: &DeckMeta) -> DeckView {
    let deck = e.deck(id);
    let (position_secs, duration_secs) = match deck.track() {
        Some(t) => {
            let rate = t.sample_rate() as f64;
            (deck.position() / rate, t.frames() as f64 / rate)
        }
        None => (0.0, 0.0),
    };
    let mut hot_cues = [false; HOT_CUES];
    for (i, slot) in hot_cues.iter_mut().enumerate() {
        *slot = deck.hot_cue_position(i).is_some();
    }
    DeckView {
        id,
        focused: id == focused,
        title: deck.track().and(meta.title.clone()),
        bpm: meta.bpm,
        key: meta.key.clone(),
        position_secs,
        duration_secs,
        playing: deck.is_playing(),
        hot_cues,
        envelope: meta.envelope.clone(),
    }
}

pub fn screen_view(e: &Engine, focused: DeckId, metas: &[DeckMeta; 2], status: String) -> ScreenView {
    ScreenView {
        decks: [
            deck_view(e, DeckId::A, focused, &metas[0]),
            deck_view(e, DeckId::B, focused, &metas[1]),
        ],
        mixer: MixerView {
            crossfader: e.crossfader(),
            faders: [e.channel_fader(DeckId::A), e.channel_fader(DeckId::B)],
            headphone_cue: [e.headphone_cue(DeckId::A), e.headphone_cue(DeckId::B)],
        },
        status,
    }
}
