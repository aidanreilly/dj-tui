//! Actions written as words, for mapping files: `play a`, `eq b high up`, `hotcue a 3`.
//!
//! A MIDI mapping is a file, so it needs names for the things a key press produces. Keeping
//! the parser here means the keyboard and a controller reach the same `Action` set.

use crate::{Action, Band, Dir};
use engine::DeckId;

fn deck(word: &str) -> Option<DeckId> {
    match word {
        "a" => Some(DeckId::A),
        "b" => Some(DeckId::B),
        _ => None,
    }
}

fn dir(word: &str) -> Option<Dir> {
    match word {
        "up" => Some(Dir::Up),
        "down" => Some(Dir::Down),
        _ => None,
    }
}

fn band(word: &str) -> Option<Band> {
    match word {
        "high" => Some(Band::High),
        "mid" => Some(Band::Mid),
        "low" => Some(Band::Low),
        _ => None,
    }
}

/// Parse one action name. Case and spacing are free, anything else is rejected rather than
/// guessed at, so a typo in a mapping file is reported instead of silently doing nothing.
pub fn parse_action(text: &str) -> Option<Action> {
    let lower = text.to_ascii_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let action = match words.as_slice() {
        ["play", d] => Action::PlayPause(deck(d)?),
        ["cue", d] => Action::CuePress(deck(d)?),
        ["cue-release", d] => Action::CueRelease(deck(d)?),
        ["focus", d] => Action::Focus(deck(d)?),
        ["hotcue", d, pad] => Action::HotCue(deck(d)?, pad_index(pad)?),
        ["hotcue-clear", d, pad] => Action::ClearHotCue(deck(d)?, pad_index(pad)?),
        ["sync", d] => Action::Sync(deck(d)?),
        ["quantize", d] => Action::Quantize(deck(d)?),
        ["keylock", d] => Action::KeyLock(deck(d)?),
        ["loop", d] => Action::LoopToggle(deck(d)?),
        ["loop-in", d] => Action::LoopIn(deck(d)?),
        ["loop-out", d] => Action::LoopOut(deck(d)?),
        ["loop-halve", d] => Action::LoopHalve(deck(d)?),
        ["loop-double", d] => Action::LoopDouble(deck(d)?),
        ["beatjump", d, way] => Action::BeatJump(deck(d)?, dir(way)?),
        ["nudge", d, way] => Action::Nudge(deck(d)?, dir(way)?),
        ["tempo", d, way, "fine"] => Action::Tempo(deck(d)?, dir(way)?, true),
        ["tempo", d, way] => Action::Tempo(deck(d)?, dir(way)?, false),
        ["seek", d, tenth] => Action::SeekTenth(deck(d)?, tenth_index(tenth)?),
        ["trim", d, way] => Action::Trim(deck(d)?, dir(way)?),
        // Before the general form, which would otherwise swallow "kill" as a band name.
        ["eq", d, "kill", b] => Action::EqKill(deck(d)?, band(b)?),
        ["eq", d, b, way] => Action::Eq(deck(d)?, band(b)?, dir(way)?),
        ["filter", d, way] => Action::Filter(deck(d)?, dir(way)?),
        ["fader", d, way] => Action::Fader(deck(d)?, dir(way)?),
        ["cue-mix", d] => Action::HeadphoneCue(deck(d)?),
        ["crossfader", way, "end"] => Action::Crossfader(dir(way)?, true),
        ["crossfader", way] => Action::Crossfader(dir(way)?, false),
        ["fx", d] => Action::FxToggle(deck(d)?),
        ["fx-next", d] => Action::FxNext(deck(d)?),
        ["fx", "wet", d, way] => Action::FxWet(deck(d)?, dir(way)?),
        ["fx", "param", d, knob, way] => Action::FxParam(deck(d)?, knob_index(knob)?, dir(way)?),
        ["waveform"] => Action::CycleWaveformMode,
        ["browser", way] => Action::BrowserMove(dir(way)?),
        ["load", d] => Action::Load(deck(d)?),
        ["search"] => Action::Search,
        ["browser-fullscreen"] => Action::BrowserFullscreen,
        ["help"] => Action::Help,
        ["quit"] => Action::Quit,
        _ => return None,
    };
    Some(action)
}

/// Pads are written as they are printed on the hardware, from one.
fn pad_index(text: &str) -> Option<usize> {
    let n: usize = text.parse().ok()?;
    (1..=engine::HOT_CUES).contains(&n).then_some(n - 1)
}

fn knob_index(text: &str) -> Option<usize> {
    let n: usize = text.parse().ok()?;
    (1..=2).contains(&n).then_some(n - 1)
}

fn tenth_index(text: &str) -> Option<u8> {
    let n: u8 = text.parse().ok()?;
    (n <= 9).then_some(n)
}
