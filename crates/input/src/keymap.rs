use crate::{Action, Band, Dir};
use engine::DeckId;

/// Terminal-independent key. The TUI converts crossterm events into these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Tab,
    Space,
    Enter,
    Esc,
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub alt: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub release: bool,
}

impl KeyEvent {
    pub fn press(key: Key) -> Self {
        Self {
            key,
            alt: false,
            shift: false,
            ctrl: false,
            release: false,
        }
    }
    pub fn release(key: Key) -> Self {
        Self {
            release: true,
            ..Self::press(key)
        }
    }
    pub fn alt(self) -> Self {
        Self { alt: true, ..self }
    }
    pub fn shift(self) -> Self {
        Self {
            shift: true,
            ..self
        }
    }
    pub fn ctrl(self) -> Self {
        Self { ctrl: true, ..self }
    }
}

const CUE_KEY: char = 'c';
const OTHER_DECK_KEY: char = '`';
const SEEK_KEY: char = 'g';

/// Focus-based keymap. One set of keys drives whichever deck has focus;
/// backtick routes the next key to the other deck without moving focus.
pub struct Keymap {
    focused: DeckId,
    next_to_other: bool,
    pending_seek: Option<DeckId>,
    cue_held: Option<DeckId>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new()
    }
}

impl Keymap {
    pub fn new() -> Self {
        Self {
            focused: DeckId::A,
            next_to_other: false,
            pending_seek: None,
            cue_held: None,
        }
    }

    pub fn focused(&self) -> DeckId {
        self.focused
    }

    pub fn handle(&mut self, e: KeyEvent) -> Option<Action> {
        if e.release {
            return match e.key {
                Key::Char(c) if c.to_ascii_lowercase() == CUE_KEY => {
                    self.cue_held.take().map(Action::CueRelease)
                }
                _ => None,
            };
        }

        if let Some(deck) = self.pending_seek.take() {
            return match e.key {
                Key::Char(c @ '0'..='9') => Some(Action::SeekTenth(deck, c as u8 - b'0')),
                _ => None,
            };
        }

        if e.key == Key::Char(OTHER_DECK_KEY) {
            self.next_to_other = !self.next_to_other;
            return None;
        }
        let target = if std::mem::take(&mut self.next_to_other) {
            self.focused.other()
        } else {
            self.focused
        };

        if e.ctrl {
            return (e.key == Key::Char('q')).then_some(Action::Quit);
        }

        match e.key {
            Key::Tab => {
                self.focused = self.focused.other();
                Some(Action::Focus(self.focused))
            }
            Key::Space => Some(Action::PlayPause(target)),
            Key::Enter => Some(Action::Load(target)),
            Key::Left => Some(Action::Crossfader(Dir::Down, e.shift)),
            Key::Right => Some(Action::Crossfader(Dir::Up, e.shift)),
            Key::Up => Some(Action::BrowserMove(Dir::Up)),
            Key::Down => Some(Action::BrowserMove(Dir::Down)),
            Key::Esc => None,
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
            return Some(if alt {
                EqKill(d, band)
            } else {
                Eq(d, band, dir(c.is_uppercase()))
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
            'f' => FxToggle(d),
            'F' => FxNext(d),
            'm' => HeadphoneCue(d),
            'r' | 'R' => Trim(d, dir(c == 'R')),
            'o' | 'O' => Filter(d, dir(c == 'O')),
            'v' | 'V' => Fader(d, dir(c == 'V')),
            '/' => Search,
            'b' => BrowserFullscreen,
            'W' => CycleWaveformMode,
            '?' => Help,
            _ => return None,
        })
    }
}
