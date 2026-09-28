//! The identity of a continuous control. Both input paths name these: MIDI knobs set them
//! outright, and the keyboard's fades ramp them. Stepping them would throw away where the
//! hardware is pointing, which is why they are set rather than nudged.

use crate::Band;
use engine::DeckId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    Crossfader,
    Fader(DeckId),
    Tempo(DeckId),
    Trim(DeckId),
    Eq(DeckId, Band),
    Filter(DeckId),
    FxWet(DeckId),
    FxParam(DeckId, usize),
    CueMix,
}

impl Control {
    /// Parse a control name: `fader a`, `eq b mid`, `fx param a 2`, `crossfader`.
    pub fn parse(text: &str) -> Option<Control> {
        let lower = text.to_ascii_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        let deck = |w: &str| match w {
            "a" => Some(DeckId::A),
            "b" => Some(DeckId::B),
            _ => None,
        };
        let band = |w: &str| match w {
            "high" => Some(Band::High),
            "mid" => Some(Band::Mid),
            "low" => Some(Band::Low),
            _ => None,
        };
        Some(match words.as_slice() {
            ["crossfader"] => Control::Crossfader,
            ["cue-mix"] => Control::CueMix,
            ["fader", d] => Control::Fader(deck(d)?),
            ["tempo", d] => Control::Tempo(deck(d)?),
            ["trim", d] => Control::Trim(deck(d)?),
            ["filter", d] => Control::Filter(deck(d)?),
            ["eq", d, b] => Control::Eq(deck(d)?, band(b)?),
            ["fx", "wet", d] => Control::FxWet(deck(d)?),
            ["fx", "param", d, knob] => {
                let n: usize = knob.parse().ok()?;
                Control::FxParam(deck(d)?, (1..=2).contains(&n).then(|| n - 1)?)
            }
            _ => return None,
        })
    }
}
