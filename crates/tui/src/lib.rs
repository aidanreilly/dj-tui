//! Terminal UI: layout, widgets and waveform rasterising. Pure rendering, driven by view structs.

pub mod pixel;
pub mod waveform;
mod graphics;
mod deck;
mod keys;
mod layout;
mod screen;

pub use deck::{waveform_area, DeckPanel, DeckView, WAVEFORM_ROWS};
pub use graphics::Graphics;
pub use keys::convert_key;
pub use layout::{screen_layout, ScreenLayout, DECK_HEIGHT};
pub use screen::{render_screen, MixerView, ScreenView};
