mod browser;
mod mix;

use crate::{Action, Dir};
use engine::DeckId;

/// Terminal-independent key. The TUI converts crossterm events into these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Tab,
    Space,
    Enter,
    Esc,
    Backspace,
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

/// Which part of the app the keyboard belongs to. A mode nobody can see is a trap, so
/// `Keymap::mode` is read every frame and drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Mix,
    Browser,
    Devices,
}

pub(super) const CUE_KEY: char = 'c';
pub(super) const SEEK_KEY: char = 'g';

/// Focus-based keymap. One set of keys drives whichever deck has focus, and `Tab` is the only
/// way to reach the other one.
pub struct Keymap {
    mode: Mode,
    focused: DeckId,
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
            mode: Mode::Mix,
            focused: DeckId::A,
            pending_seek: None,
            cue_held: None,
        }
    }

    pub fn focused(&self) -> DeckId {
        self.focused
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn handle(&mut self, e: KeyEvent) -> Option<Action> {
        // Cue is the only held key, and its release belongs to the deck that was pressed.
        if e.release {
            return match e.key {
                Key::Char(c) if c.to_ascii_lowercase() == CUE_KEY => {
                    self.cue_held.take().map(Action::CueRelease)
                }
                _ => None,
            };
        }

        // Quit is reachable from everywhere; nothing else global is.
        if e.ctrl && e.key == Key::Char('q') {
            return Some(Action::Quit);
        }

        // Tab picks the deck a load or a deck key lands on, in every mode.
        if e.key == Key::Tab {
            self.focused = self.focused.other();
            return Some(Action::Focus(self.focused));
        }

        let action = match self.mode {
            Mode::Mix => self.mix(e),
            Mode::Browser => browser::handle(e, self.focused),
            Mode::Devices => devices(e),
        }?;

        // The mode changes as a consequence of the action, so there is one place to look.
        match action {
            Action::BrowserEnter | Action::Search => self.mode = Mode::Browser,
            Action::BrowserLeave => self.mode = Mode::Mix,
            Action::Devices => self.mode = Mode::Devices,
            Action::DeviceChoose | Action::DeviceClose => self.mode = Mode::Mix,
            _ => {}
        }
        Some(action)
    }
}

/// The device chooser: a list and a way out, and nothing else.
fn devices(e: KeyEvent) -> Option<Action> {
    match e.key {
        Key::Up => Some(Action::DeviceMove(Dir::Up)),
        Key::Down => Some(Action::DeviceMove(Dir::Down)),
        Key::Enter => Some(Action::DeviceChoose),
        Key::Esc => Some(Action::DeviceClose),
        Key::Char('d') if e.ctrl => Some(Action::DeviceClose),
        _ => None,
    }
}
