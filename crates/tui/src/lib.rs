//! Terminal UI: layout, widgets and waveform rasterising. Pure rendering, driven by view structs.

mod deck;
mod graphics;
mod keys;
mod layout;
pub mod pixel;
mod screen;
pub mod waveform;

pub use deck::{waveform_area, DeckPanel, DeckView, WAVEFORM_ROWS};
pub use graphics::{detect_graphics, multiplexer_detected, Graphics};
pub use keys::convert_key;
pub use layout::{screen_layout, ScreenLayout, DECK_HEIGHT};
pub use screen::{render_screen, MixerView, ScreenView};
