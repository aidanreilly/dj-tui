//! Terminal UI: layout, widgets and waveform rasterising. Pure rendering, driven by view structs.

pub mod waveform;
mod deck;
mod layout;

pub use deck::{DeckPanel, DeckView};
pub use layout::{screen_layout, ScreenLayout};
