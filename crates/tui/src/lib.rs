//! Terminal UI: layout, widgets and waveform rasterising. Pure rendering, driven by view structs.

mod browser;
mod deck;
mod graphics;
mod keys;
mod layout;
pub mod mixer;
pub mod pixel;
mod screen;
pub mod theme;
pub mod waveform;

pub use browser::{BrowserPanel, BrowserRow, BrowserView};
pub use deck::{
    waveform_area, DeckPanel, DeckView, END_WARNING_COLOUR, HOT_CUE_COLOURS, LOOP_COLOUR,
    MAIN_CUE_COLOUR, WAVEFORM_ROWS,
};
pub use graphics::{detect_graphics, multiplexer_detected, Graphics};
pub use keys::convert_key;
pub use layout::{screen_layout, ScreenLayout, DECK_HEIGHT};
pub use mixer::{MixerView, StripView};
pub use screen::{render_screen, DeviceView, ScreenView};
