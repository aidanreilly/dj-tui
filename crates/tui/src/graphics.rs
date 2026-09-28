//! Draws pixel waveforms over the glyph ones on terminals with a bitmap protocol.
//! Created only when detection found one; everything else keeps the glyph renderer.

use crate::pixel::{loop_columns, playhead_x, Palette, PixelWaveform, Wave};
use crate::{screen_layout, waveform_area, ScreenView};
use image::DynamicImage;
use ratatui::{layout::Size, Frame};
use ratatui_image::{
    picker::{Capability, Picker, ProtocolType},
    protocol::{kitty::Kitty, Protocol},
    Image, Resize,
};

/// Fixed kitty image ids, one per deck, so each new image replaces the last in the
/// terminal's image store instead of accumulating there.
const KITTY_IDS: [u32; 2] = [0x00D7_0A01, 0x00D7_0B01];

#[derive(Default)]
struct DeckGraphics {
    pixel: PixelWaveform,
    protocol: Option<Protocol>,
}

pub struct Graphics {
    picker: Picker,
    palette: Palette,
    decks: [DeckGraphics; 2],
    transmissions: u64,
}

impl Graphics {
    pub fn new(picker: Picker) -> Self {
        Self {
            picker,
            palette: Palette::default(),
            decks: Default::default(),
            transmissions: 0,
        }
    }

    /// Call right after `render_screen` with the same view.
    pub fn render(&mut self, frame: &mut Frame, view: &ScreenView) {
        // A kitty image is not a cell attribute, so nothing drawn over it afterwards hides
        // it: the full-screen browser, the help overlay and the device screen all have to
        // keep it off the screen in the first place.
        if covers_the_decks(view) {
            return;
        }
        let layout = screen_layout(frame.area());
        let font = self.picker.font_size();
        for (i, panel) in [layout.deck_a, layout.deck_b].into_iter().enumerate() {
            let area = waveform_area(panel);
            let dv = &view.decks[i];
            let px = (
                area.width as u32 * font.width as u32,
                area.height as u32 * font.height as u32,
            );
            let ranges: &[[f32; 2]] = if dv.title.is_some() {
                &dv.waveform
            } else {
                &[]
            };
            let playhead = playhead_x(dv.position_secs, dv.duration_secs, px.0);
            let deck = &mut self.decks[i];
            let wave = Wave {
                ranges,
                bands: &dv.bands,
                mode: dv.waveform_mode,
                warning: dv.end_warning,
                loop_cols: loop_columns(dv.loop_secs, dv.duration_secs, px.0),
                focused: dv.focused,
            };
            match deck.pixel.update(&wave, px, playhead, &self.palette) {
                Some(img) => {
                    let size = Size::new(area.width, area.height);
                    deck.protocol = make_protocol(
                        &self.picker,
                        DynamicImage::ImageRgba8(img),
                        size,
                        KITTY_IDS[i],
                    );
                    self.transmissions += 1;
                }
                None if !deck.pixel.has_image() => deck.protocol = None,
                None => {}
            }
            if let Some(p) = &deck.protocol {
                frame.render_widget(Image::new(p), area);
            }
        }
    }

    /// Images handed to the terminal so far.
    pub fn transmissions(&self) -> u64 {
        self.transmissions
    }
}

/// Whether something is drawn where the deck panels would be.
fn covers_the_decks(view: &ScreenView) -> bool {
    view.browser.fullscreen || view.help || view.devices.is_some()
}

fn make_protocol(picker: &Picker, img: DynamicImage, size: Size, id: u32) -> Option<Protocol> {
    match picker.protocol_type() {
        ProtocolType::Kitty => {
            let compress = picker
                .capabilities()
                .contains(&Capability::KittyCompression);
            Kitty::new(img, size, id, picker.tmux_detected(), compress)
                .ok()
                .map(Protocol::Kitty)
        }
        _ => picker.new_protocol(img, size, Resize::Fit(None)).ok(),
    }
}

/// True inside tmux, GNU screen or zellij, which don't pass kitty graphics through reliably.
pub fn multiplexer_detected(env: impl Fn(&str) -> Option<String>) -> bool {
    env("TMUX").is_some()
        || env("STY").is_some()
        || env("ZELLIJ").is_some()
        || env("TERM").is_some_and(|t| t.starts_with("screen") || t.starts_with("tmux"))
}

/// Ask the terminal for kitty graphics support. Call once, after raw mode is on.
/// Returns `None` when the terminal lacks it or a multiplexer sits in between.
pub fn detect_graphics() -> Option<Picker> {
    if multiplexer_detected(|k| std::env::var(k).ok()) {
        return None;
    }
    let options = ratatui_image::picker::cap_parser::QueryStdioOptions {
        kitty_compression: true,
        ..Default::default()
    };
    let picker = Picker::from_query_stdio_with_options(options).ok()?;
    (picker.protocol_type() == ProtocolType::Kitty).then_some(picker)
}
