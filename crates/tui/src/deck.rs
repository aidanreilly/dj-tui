use crate::pixel::Palette;
use crate::waveform::{amplitude_rows, band_colors, downsample_points, ranges};
use engine::DeckId;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Widget},
};

pub const WAVEFORM_ROWS: u16 = 8;

/// Where a deck panel draws its waveform: the rows under the title, inside the border.
pub fn waveform_area(panel: Rect) -> Rect {
    let inner = Block::bordered().inner(panel);
    Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        WAVEFORM_ROWS.min(inner.height.saturating_sub(1)),
    )
}

/// Everything the deck panel needs to draw, copied out of engine state each frame.
#[derive(Debug, Clone)]
pub struct DeckView {
    pub id: DeckId,
    pub focused: bool,
    pub title: Option<String>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub loading: bool,
    pub playing: bool,
    pub hot_cues: [bool; 8],
    /// Whole-track analysis points, resampled to panel width when drawn.
    pub waveform: Vec<wave::WavePoint>,
}

pub struct DeckPanel<'a> {
    view: &'a DeckView,
    palette: &'a Palette,
}

static DEFAULT_PALETTE: std::sync::OnceLock<Palette> = std::sync::OnceLock::new();

impl<'a> DeckPanel<'a> {
    /// Draws in the default palette. Kept for callers that have no mode to hand.
    pub fn new(view: &'a DeckView) -> Self {
        Self::with_palette(view, DEFAULT_PALETTE.get_or_init(Palette::default))
    }

    pub fn with_palette(view: &'a DeckView, palette: &'a Palette) -> Self {
        Self { view, palette }
    }
}

fn mmss(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

impl Widget for DeckPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let v = self.view;
        let letter = match v.id {
            DeckId::A => 'A',
            DeckId::B => 'B',
        };
        let (label, border) = if v.focused {
            (format!(" ▶ DECK {letter} "), BorderType::Thick)
        } else {
            (format!(" DECK {letter} "), BorderType::Plain)
        };
        let info = match (v.bpm, &v.key) {
            (Some(bpm), Some(key)) => format!(" {bpm:.2}  {key} "),
            (Some(bpm), None) => format!(" {bpm:.2} "),
            _ => String::new(),
        };
        let mut block = Block::bordered()
            .border_type(border)
            .title(Line::from(label))
            .title(Line::from(info).right_aligned());
        if v.focused {
            block = block.border_style(Style::new().add_modifier(Modifier::BOLD));
        }
        let inner = block.inner(area);
        block.render(area, buf);
        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // Title row.
        let Some(title) = &v.title else {
            let (text, style) = if v.loading {
                (
                    "Loading track…",
                    Style::new().fg(ratatui::style::Color::Rgb(126, 113, 190)),
                )
            } else {
                ("No track loaded", Style::new().add_modifier(Modifier::DIM))
            };
            buf.set_string(inner.x, inner.y, text, style);
            return;
        };
        let times = format!(
            "{}  -{}",
            mmss(v.position_secs),
            mmss(v.duration_secs - v.position_secs)
        );
        buf.set_stringn(inner.x, inner.y, title, inner.width as usize, Style::new());
        let tx = inner.right().saturating_sub(times.chars().count() as u16);
        buf.set_string(tx, inner.y, &times, Style::new());

        // Waveform rows.
        let wave_y = inner.y + 1;
        let columns = downsample_points(&v.waveform, inner.width as usize);
        let rows = amplitude_rows(&ranges(&columns), WAVEFORM_ROWS as usize);
        let colours = band_colors(&columns, WAVEFORM_ROWS as usize, self.palette);
        let center_y = wave_y + WAVEFORM_ROWS / 2;
        buf.set_string(
            inner.x,
            center_y,
            "┄".repeat(inner.width as usize),
            Style::new().fg(ratatui::style::Color::Rgb(58, 50, 84)),
        );
        for (i, row) in rows.iter().enumerate() {
            let y = wave_y + i as u16;
            if y >= inner.bottom() {
                break;
            }
            for (col, glyph) in row.chars().enumerate().filter(|(_, glyph)| *glyph != ' ') {
                let colour = colours
                    .get(i)
                    .and_then(|r| r.get(col))
                    .copied()
                    .unwrap_or(ratatui::style::Color::Rgb(126, 113, 190));
                buf.set_string(inner.x + col as u16, y, glyph.to_string(), Style::new().fg(colour));
            }
        }
        if v.duration_secs > 0.0 {
            let frac = (v.position_secs / v.duration_secs).clamp(0.0, 1.0);
            let head = inner.x + ((inner.width as f64 * frac) as u16).min(inner.width - 1);
            for y in wave_y..(wave_y + WAVEFORM_ROWS).min(inner.bottom()) {
                for x in inner.x..head {
                    buf[(x, y)].modifier.insert(Modifier::DIM);
                }
                // Draw the playhead after the waveform using the same half-cell bar width.
                buf.set_string(
                    head,
                    y,
                    "▌",
                    Style::new().fg(ratatui::style::Color::Rgb(169, 165, 145)),
                );
            }
        }

        // Status row: hot cues and transport state.
        let status_y = wave_y + WAVEFORM_ROWS + 1;
        if status_y < inner.bottom() {
            let cues: String = v
                .hot_cues
                .iter()
                .enumerate()
                .map(|(i, &set)| {
                    if set {
                        format!("[{}]", i + 1)
                    } else {
                        "[ ]".into()
                    }
                })
                .collect();
            let state = if v.playing { "PLAYING" } else { "PAUSED" };
            buf.set_string(inner.x, status_y, format!("{cues}   {state}"), Style::new());
        }
    }
}
