//! Every colour the UI draws, in one place.
//!
//! The palette is Solarized (Ethan Schoonover, <https://ethanschoonover.com/solarized/>),
//! whose eight accents and eight greys were picked to hold their relationships to each other
//! under a terminal's own contrast. Nothing here sets a background: the app draws on whatever
//! the terminal has, so the accents are the whole of the scheme and a light terminal stays
//! usable.
//!
//! The constants below the palette name roles rather than colours, so a widget asks for
//! [`FOCUS_BORDER`] rather than for red and the mapping lives here.

use ratatui::style::Color;

// --- Solarized accents ---

pub const YELLOW: [u8; 3] = [181, 137, 0];
pub const ORANGE: [u8; 3] = [203, 75, 22];
pub const RED: [u8; 3] = [220, 50, 47];
pub const MAGENTA: [u8; 3] = [211, 54, 130];
pub const VIOLET: [u8; 3] = [108, 113, 196];
pub const BLUE: [u8; 3] = [38, 139, 210];
pub const CYAN: [u8; 3] = [42, 161, 152];
pub const GREEN: [u8; 3] = [133, 153, 0];

// --- Solarized base tones, darkest to lightest ---

pub const BASE03: [u8; 3] = [0, 43, 54];
pub const BASE02: [u8; 3] = [7, 54, 66];
pub const BASE01: [u8; 3] = [88, 110, 117];
pub const BASE00: [u8; 3] = [101, 123, 131];
pub const BASE0: [u8; 3] = [131, 148, 150];
pub const BASE1: [u8; 3] = [147, 161, 161];
pub const BASE2: [u8; 3] = [238, 232, 213];
pub const BASE3: [u8; 3] = [253, 246, 227];

// --- Roles ---

/// Border of the deck the keys are acting on.
pub const FOCUS_BORDER: [u8; 3] = RED;
/// Title of a focused deck panel, which sits back from the text under it.
pub const PANEL_TITLE: [u8; 3] = BASE01;
/// Border of the browser while it has the keyboard.
pub const BROWSER_BORDER: [u8; 3] = CYAN;
/// A killed EQ band's marker, and the panel with no track in it.
pub const DIM_LABEL: [u8; 3] = BASE01;
/// The label a deck shows while a track is being read.
pub const LOADING: [u8; 3] = VIOLET;
/// Zero line through the middle of the waveform.
pub const CENTRE_LINE: [u8; 3] = BASE02;
/// Playhead in the pixel renderer, which is the brightest thing on screen.
pub const PLAYHEAD: [u8; 3] = BASE3;
/// Playhead in the glyph renderer, a step down so a whole column of it does not glare.
pub const GLYPH_PLAYHEAD: [u8; 3] = BASE1;
/// Background behind the glyph waveform inside an active loop.
pub const LOOP_REGION_BG: [u8; 3] = BASE02;
/// A column with no band data behind it in RGB mode.
pub const RGB_FALLBACK: [u8; 3] = BASE1;
/// Meter segments, quiet to clipping.
pub const METER_OK: [u8; 3] = GREEN;
pub const METER_HOT: [u8; 3] = YELLOW;
pub const METER_CLIP: [u8; 3] = RED;
/// A track whose key would mix with what is playing.
pub const KEY_MATCH: [u8; 3] = GREEN;

/// Turn one of the constants above into a ratatui colour.
pub const fn colour(c: [u8; 3]) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}

/// Turn one of the constants above into an opaque image pixel.
pub const fn pixel(c: [u8; 3]) -> image::Rgba<u8> {
    image::Rgba([c[0], c[1], c[2], 255])
}
