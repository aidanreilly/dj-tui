//! Input layer: keyboard (and later MIDI) events become engine-independent `Action`s.

mod action;
mod keymap;
mod names;

pub use action::{Action, Band, Dir};
pub use keymap::{Key, KeyEvent, Keymap};
pub use names::parse_action;
