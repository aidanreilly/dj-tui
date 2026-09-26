//! Input layer: keyboard (and later MIDI) events become engine-independent `Action`s.

mod action;
mod keymap;

pub use action::{Action, Band, Dir};
pub use keymap::{Key, KeyEvent, Keymap};
