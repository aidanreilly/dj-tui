//! Terminal UI: layout, widgets and waveform rasterising. Pure rendering, driven by view structs.

pub mod waveform;
mod deck;
mod keys;
mod layout;
mod screen;

pub use deck::{DeckPanel, DeckView, WAVEFORM_ROWS};
pub use keys::convert_key;
pub use layout::{screen_layout, ScreenLayout, DECK_HEIGHT};
pub use screen::{render_screen, MixerView, ScreenView};
