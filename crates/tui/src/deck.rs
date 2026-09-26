use crate::waveform::{bar_rows, downsample_peaks};
use engine::DeckId;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Widget},
};

pub const WAVEFORM_ROWS: u16 = 8;

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
    pub playing: bool,
    pub hot_cues: [bool; 8],
    /// Whole-track peak envelope, any resolution; resampled to the panel width.
    pub envelope: Vec<f32>,
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
            buf.set_string(inner.x, inner.y, "No track loaded", Style::new().add_modifier(Modifier::DIM));
            return;
        };
        let times = format!("{}  -{}", mmss(v.position_secs), mmss(v.duration_secs - v.position_secs));
        buf.set_stringn(inner.x, inner.y, title, inner.width as usize, Style::new());
        let tx = inner.right().saturating_sub(times.chars().count() as u16);
        buf.set_string(tx, inner.y, &times, Style::new());

        // Waveform rows.
        let wave_y = inner.y + 1;
        let rows = bar_rows(&downsample_peaks(&v.envelope, inner.width as usize), WAVEFORM_ROWS as usize);
        for (i, row) in rows.iter().enumerate() {
            let y = wave_y + i as u16;
            if y >= inner.bottom() {
                break;
            }
            buf.set_string(inner.x, y, row, Style::new());
        }
        if v.duration_secs > 0.0 {
            let frac = (v.position_secs / v.duration_secs).clamp(0.0, 1.0);
            let head = inner.x + ((inner.width as f64 * frac) as u16).min(inner.width - 1);
            for y in wave_y..(wave_y + WAVEFORM_ROWS).min(inner.bottom()) {
                for x in inner.x..head {
                    buf[(x, y)].modifier.insert(Modifier::DIM);
                }
                buf[(head, y)].modifier.insert(Modifier::REVERSED);
            }
        }

        // Status row: hot cues and transport state.
        let status_y = wave_y + WAVEFORM_ROWS + 1;
        if status_y < inner.bottom() {
            let cues: String = v
                .hot_cues
                .iter()
                .enumerate()
                .map(|(i, &set)| if set { format!("[{}]", i + 1) } else { "[ ]".into() })
                .collect();
            let state = if v.playing { "PLAYING" } else { "PAUSED" };
            buf.set_string(inner.x, status_y, format!("{cues}   {state}"), Style::new());
        }
    }
}
