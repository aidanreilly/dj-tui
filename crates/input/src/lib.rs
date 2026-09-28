//! Input layer: keyboard (and later MIDI) events become engine-independent `Action`s.

mod action;
mod control;
mod keymap;
mod names;

pub use action::{Action, Band, Dir};
pub use control::Control;
pub use keymap::{Key, KeyEvent, Keymap, Mode};
pub use names::parse_action;
