//! Mix mode: the whole keyboard is transport, mixer and effects, acting on the focused deck.

use super::{Key, KeyEvent, Keymap, CUE_KEY, SEEK_KEY};
use crate::{Action, Band, Dir};
use engine::DeckId;

impl Keymap {
    pub(super) fn mix(&mut self, e: KeyEvent) -> Option<Action> {
        if let Some(deck) = self.pending_seek.take() {
            return match e.key {
                Key::Char(c @ '0'..='9') => Some(Action::SeekTenth(deck, c as u8 - b'0')),
                _ => None,
            };
        }

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
            Key::Left => Some(Action::Crossfader(Dir::Down, e.shift)),
            Key::Right => Some(Action::Crossfader(Dir::Up, e.shift)),
            Key::Up => Some(Action::BrowserMove(Dir::Up)),
            Key::Down => Some(Action::BrowserMove(Dir::Down)),
            Key::Esc | Key::Backspace | Key::Tab => None,
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
                self.cue_held = Some(d);
                CuePress(d)
            }
            SEEK_KEY => {
                self.pending_seek = Some(d);
                return None;
            }
            '+' | '=' => Tempo(d, Dir::Up, alt),
            '-' => Tempo(d, Dir::Down, alt),
            ',' | '.' => Nudge(d, dir(c == '.')),
            '<' | '>' => BeatJump(d, dir(c == '>')),
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
            'o' | 'O' => Filter(d, dir(c == 'O')),
            'v' | 'V' => Fader(d, dir(c == 'V')),
            '/' => Search,
            'b' => BrowserEnter,
            'S' => BrowserSort(alt),
            'A' => AnalyseLibrary,
            'w' | 'W' => CycleWaveformMode,
            '?' => Help,
            _ => return None,
        })
    }
}
