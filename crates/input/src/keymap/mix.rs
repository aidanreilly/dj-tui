//! Mix mode: the whole keyboard is transport, mixer and effects, acting on the focused deck.

use super::{Key, KeyEvent, Keymap, CUE_KEY, SEEK_KEY};
use crate::{Action, Band, Dir};
use engine::DeckId;

impl Keymap {
    pub(super) fn mix(&mut self, e: KeyEvent) -> Option<Action> {
        let target = self.focused;

        if e.ctrl {
            return match e.key {
                Key::Char('d') => Some(Action::Devices),
                _ => None,
            };
        }

        match e.key {
            Key::Space => Some(Action::PlayPause(target)),
            Key::Enter => Some(Action::Load(target)),
            // Horizontal arrows are the horizontal fader, vertical arrows the vertical one
            // belonging to whichever deck has focus. Shift goes hard to an end, Alt fades.
            Key::Left if e.alt => Some(Action::CrossfaderFade(Dir::Down)),
            Key::Right if e.alt => Some(Action::CrossfaderFade(Dir::Up)),
            Key::Left => Some(Action::Crossfader(Dir::Down, e.shift)),
            Key::Right => Some(Action::Crossfader(Dir::Up, e.shift)),
            Key::Up if e.alt => Some(Action::FaderFade(target, Dir::Up)),
            Key::Down if e.alt => Some(Action::FaderFade(target, Dir::Down)),
            Key::Up if e.shift => Some(Action::FaderEnd(target, Dir::Up)),
            Key::Down if e.shift => Some(Action::FaderEnd(target, Dir::Down)),
            Key::Up => Some(Action::Fader(target, Dir::Up)),
            Key::Down => Some(Action::Fader(target, Dir::Down)),
            Key::Esc => Some(Action::CancelFades),
            Key::Backspace | Key::Tab => None,
            Key::Char(c) => self.char_action(c, e.alt, target),
        }
    }

    fn char_action(&mut self, c: char, alt: bool, d: DeckId) -> Option<Action> {
        use Action::*;
        let dir = |up: bool| if up { Dir::Up } else { Dir::Down };
        let eq_band = |c: char| match c.to_ascii_lowercase() {
            't' => Some(Band::High),
            'y' => Some(Band::Mid),
            'u' => Some(Band::Low),
            _ => None,
        };

        if let Some(band) = eq_band(c) {
            // Stepped EQ gain is a MIDI control. The keyboard gets the two gestures it does
            // well: kill a band, or hand it to the focused deck.
            if alt {
                return None;
            }
            return Some(if c.is_uppercase() {
                EqSwap(d, band)
            } else {
                EqKill(d, band)
            });
        }

        Some(match c {
            '1'..='8' => {
                let n = (c as u8 - b'1') as usize;
                if alt {
                    ClearHotCue(d, n)
                } else {
                    HotCue(d, n)
                }
            }
            '9' | '0' => FxWet(d, dir(c == '0')),
            CUE_KEY => {
                // Auto-repeat arrives as more presses. Pressing cue again while it is held
                // would re-seek to the cue point over and over.
                if self.cue_held.is_some() {
                    return None;
                }
                self.cue_held = Some(d);
                CuePress(d)
            }
            SEEK_KEY => {
                self.pending_seek = Some(d);
                return None;
            }
            // The unmodified pair rides both decks together, the way you would ride two
            // pitch faders; the focused deck's own pitch is one key to the left.
            '+' | '=' => GlobalTempo(Dir::Up, alt),
            '-' => GlobalTempo(Dir::Down, alt),
            ',' | '.' => Tempo(d, dir(c == '.'), alt),
            '<' | '>' => Nudge(d, dir(c == '>')),
            'j' | 'J' => BeatJump(d, dir(c == 'J')),
            '[' => LoopHalve(d),
            ']' => LoopDouble(d),
            's' => Sync(d),
            'q' => Quantize(d),
            'k' => KeyLock(d),
            'l' => LoopToggle(d),
            'i' => LoopIn(d),
            'I' => LoopOut(d),
            'f' => FxToggle(d),
            'p' | 'P' => FxParam(d, 0, dir(c == 'P')),
            'd' | 'D' => FxParam(d, 1, dir(c == 'D')),
            'F' => FxNext(d),
            'm' => HeadphoneCue(d),
            'h' | 'H' => CueMix(dir(c == 'H')),
            'r' | 'R' => Trim(d, dir(c == 'R')),
            // Alt on a continuous control's key means do it slowly.
            'o' | 'O' => {
                if alt {
                    FilterSweep(d, dir(c == 'O'))
                } else {
                    Filter(d, dir(c == 'O'))
                }
            }
            'x' => CrossfaderCentre,
            'X' => CrossfaderFadeCentre,
            'v' => FilterCentre(d),
            'V' => FilterSweepCentre(d),
            '{' => FadeLength(Dir::Down),
            '}' => FadeLength(Dir::Up),
            '/' => Search,
            'b' => BrowserEnter,
            'w' | 'W' => CycleWaveformMode,
            '?' => Help,
            _ => return None,
        })
    }
}
