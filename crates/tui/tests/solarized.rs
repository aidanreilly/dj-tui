//! The palette is Ethan Schoonover's Solarized, and every colour the UI draws comes from it.
//! These assertions pin the sixteen values and the role each one plays, so a colour added by
//! hand somewhere else shows up here as a mismatch rather than as a slightly-off screen.

use image::Rgba;
use tui::pixel::Palette;
use tui::theme;

const YELLOW: [u8; 3] = [181, 137, 0];
const ORANGE: [u8; 3] = [203, 75, 22];
const RED: [u8; 3] = [220, 50, 47];
const MAGENTA: [u8; 3] = [211, 54, 130];
const VIOLET: [u8; 3] = [108, 113, 196];
const BLUE: [u8; 3] = [38, 139, 210];
const CYAN: [u8; 3] = [42, 161, 152];
const GREEN: [u8; 3] = [133, 153, 0];
const BASE02: [u8; 3] = [7, 54, 66];
const BASE01: [u8; 3] = [88, 110, 117];
const BASE1: [u8; 3] = [147, 161, 161];
const BASE2: [u8; 3] = [238, 232, 213];
const BASE3: [u8; 3] = [253, 246, 227];

#[test]
fn the_constants_are_the_published_solarized_values() {
    assert_eq!(theme::YELLOW, YELLOW);
    assert_eq!(theme::ORANGE, ORANGE);
    assert_eq!(theme::RED, RED);
    assert_eq!(theme::MAGENTA, MAGENTA);
    assert_eq!(theme::VIOLET, VIOLET);
    assert_eq!(theme::BLUE, BLUE);
    assert_eq!(theme::CYAN, CYAN);
    assert_eq!(theme::GREEN, GREEN);
    assert_eq!(theme::BASE03, [0, 43, 54]);
    assert_eq!(theme::BASE02, BASE02);
    assert_eq!(theme::BASE01, BASE01);
    assert_eq!(theme::BASE00, [101, 123, 131]);
    assert_eq!(theme::BASE0, [131, 148, 150]);
    assert_eq!(theme::BASE1, BASE1);
    assert_eq!(theme::BASE2, BASE2);
    assert_eq!(theme::BASE3, BASE3);
}

#[test]
fn the_waveform_palette_is_solarized() {
    let p = Palette::default();
    let rgba = |c: [u8; 3]| Rgba([c[0], c[1], c[2], 255]);
    assert_eq!(p.three_band, [rgba(BLUE), rgba(YELLOW), rgba(BASE3)]);
    assert_eq!(p.blue_dark, rgba(BLUE));
    assert_eq!(p.blue_bright, rgba(BASE2));
    assert_eq!(p.centre_line, rgba(BASE02));
    assert_eq!(p.playhead, rgba(BASE3));
    assert_eq!(p.warning, rgba(RED));
    assert_eq!(p.loop_region, rgba(GREEN));
}

#[test]
fn the_eight_hot_cues_take_the_eight_accents() {
    assert_eq!(
        tui::HOT_CUE_COLOURS,
        [YELLOW, ORANGE, RED, MAGENTA, VIOLET, BLUE, CYAN, GREEN]
    );
}

#[test]
fn the_markers_keep_their_meanings() {
    assert_eq!(tui::MAIN_CUE_COLOUR, ORANGE, "the cue point is orange");
    assert_eq!(tui::LOOP_COLOUR, GREEN, "a loop is green");
    assert_eq!(tui::END_WARNING_COLOUR, RED, "running out of track is red");
}

#[test]
fn the_glyph_renderer_draws_from_the_same_constants() {
    assert_eq!(theme::CENTRE_LINE, BASE02);
    assert_eq!(theme::GLYPH_PLAYHEAD, BASE1);
    assert_eq!(theme::LOOP_REGION_BG, BASE02);
    assert_eq!(theme::DIM_LABEL, BASE01);
    assert_eq!(theme::LOADING, VIOLET);
    assert_eq!(theme::FOCUS_BORDER, RED);
    assert_eq!(theme::PANEL_TITLE, BASE01);
    assert_eq!(theme::BROWSER_BORDER, CYAN);
}
