use crate::pixel::{colour_at, extent, Palette, WaveformMode};
use crate::waveform::{amplitude_rows, downsample_bands, downsample_ranges};
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
    /// Whole-track signed `[minimum, maximum]` sample ranges, resampled to panel width.
    pub waveform: Vec<[f32; 2]>,
    /// Peak `[low, mid, high]` per overview position; empty when not analysed.
    pub bands: Vec<[f32; 3]>,
    pub waveform_mode: WaveformMode,
    /// Bar and beat-in-bar at the playhead, when a beat grid exists.
    pub beat: Option<(i64, i64)>,
    pub cue_secs: Option<f64>,
    pub hot_cue_secs: [Option<f64>; 8],
    /// Active loop as (start, end) in seconds, drawn over the overview.
    pub loop_secs: Option<(f64, f64)>,
    /// True while cues, loops and jumps snap to the beat grid.
    pub quantize: bool,
    /// Flash phase of the end-of-track warning: true while the unplayed part shows red.
    pub end_warning: bool,
}

/// Hot cue colours, one per pad, following the CDJ palette's spread of hues.
pub const HOT_CUE_COLOURS: [[u8; 3]; 8] = [
    [40, 226, 20],
    [16, 177, 118],
    [48, 90, 255],
    [170, 114, 255],
    [255, 18, 123],
    [230, 40, 40],
    [255, 140, 20],
    [224, 224, 30],
];
pub const MAIN_CUE_COLOUR: [u8; 3] = [255, 120, 0];
pub const END_WARNING_COLOUR: [u8; 3] = [230, 30, 30];

fn rgb(c: [u8; 3]) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(c[0], c[1], c[2])
}

pub struct DeckPanel<'a> {
    view: &'a DeckView,
}

impl<'a> DeckPanel<'a> {
    pub fn new(view: &'a DeckView) -> Self {
        Self { view }
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
        let columns = downsample_ranges(&v.waveform, inner.width as usize);
        let rows = amplitude_rows(&columns, WAVEFORM_ROWS as usize);
        let bands = downsample_bands(&v.bands, inner.width as usize);
        let palette = Palette::default();
        let half_rows = WAVEFORM_ROWS as f32 / 2.0;
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
            // Colour each cell from its centre, like the pixel renderer does per row.
            let d = (i as f32 + 0.5 - half_rows).abs() / half_rows;
            for (col, glyph) in row.chars().enumerate().filter(|(_, glyph)| *glyph != ' ') {
                let peak = columns[col][1];
                let b = bands[col];
                let reach = extent(v.waveform_mode, peak, b);
                let c = colour_at(v.waveform_mode, peak, b, d.min(reach), &palette)
                    .unwrap_or(palette.three_band[0]);
                buf.set_string(
                    inner.x + col as u16,
                    y,
                    glyph.to_string(),
                    Style::new().fg(ratatui::style::Color::Rgb(c[0], c[1], c[2])),
                );
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

        if v.end_warning && v.duration_secs > 0.0 {
            let frac = (v.position_secs / v.duration_secs).clamp(0.0, 1.0);
            let head = inner.x + ((inner.width as f64 * frac) as u16).min(inner.width - 1);
            for y in wave_y..(wave_y + WAVEFORM_ROWS).min(inner.bottom()) {
                for x in head + 1..inner.right() {
                    if buf[(x, y)].symbol() != " " {
                        buf[(x, y)].set_fg(rgb(END_WARNING_COLOUR));
                    }
                }
            }
        }

        // Marker row: main cue, then hot cues on top in their colours.
        let marker_y = wave_y + WAVEFORM_ROWS;
        if marker_y < inner.bottom() && v.duration_secs > 0.0 {
            let column = |secs: f64| {
                let frac = (secs / v.duration_secs).clamp(0.0, 1.0);
                inner.x + ((inner.width as f64 * frac) as u16).min(inner.width - 1)
            };
            if let Some(cue) = v.cue_secs {
                buf.set_string(
                    column(cue),
                    marker_y,
                    "▲",
                    Style::new().fg(rgb(MAIN_CUE_COLOUR)),
                );
            }
            for (i, secs) in v.hot_cue_secs.iter().enumerate() {
                let Some(secs) = secs else { continue };
                let x = column(*secs);
                let style = Style::new().fg(rgb(HOT_CUE_COLOURS[i]));
                buf.set_string(x, marker_y, "▲", style);
                if x + 1 < inner.right() {
                    buf.set_string(x + 1, marker_y, (i + 1).to_string(), style);
                }
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
            let bar = v
                .beat
                .map(|(bar, beat)| format!("   BAR {bar}.{beat}"))
                .unwrap_or_default();
            buf.set_string(
                inner.x,
                status_y,
                format!("{cues}   {state}{bar}"),
                Style::new(),
            );
        }
    }
}
