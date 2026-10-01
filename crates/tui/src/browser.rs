//! The track list: what is in the music folder, what is known about it, and what would mix.

use crate::theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Widget},
};

/// One track as the list shows it.
#[derive(Debug, Clone, Default)]
pub struct BrowserRow {
    pub name: String,
    pub bpm: Option<f64>,
    /// Camelot code, e.g. "8A".
    pub key: Option<String>,
    pub duration_secs: Option<f64>,
    /// Its key sits next to the playing deck's on the wheel.
    pub compatible: bool,
    /// It has been analysed, so its columns are filled in.
    pub analysed: bool,
}

/// Everything the browser panel needs to draw.
#[derive(Debug, Clone, Default)]
pub struct BrowserView {
    pub rows: Vec<BrowserRow>,
    pub selected: usize,
    /// Column the list is ordered by, for the title.
    pub sort: &'static str,
    pub ascending: bool,
    /// The query while it is filtering, shown under the list.
    pub search: Option<String>,
    /// Browser mode holds the keyboard: letters type and Alt plus a letter is a command.
    pub active: bool,
    pub fullscreen: bool,
    /// What the list holds, or what it is busy doing.
    pub status: String,
}

/// What browser mode offers, on the panel's own bottom row. Six bindings document themselves
/// better here than in an overlay nobody opens.
const HINT: &str =
    "Esc back  Enter load  Tab deck  Ctrl+u clear  Alt+s sort  Alt+a analyse  Alt+f size";

/// Width of each fixed column, and the gap between them.
const BPM_WIDTH: usize = 6;
const KEY_WIDTH: usize = 3;
const LEN_WIDTH: usize = 6;
const GAP: usize = 2;

fn mmss(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

pub struct BrowserPanel<'a> {
    view: &'a BrowserView,
}

impl<'a> BrowserPanel<'a> {
    pub fn new(view: &'a BrowserView) -> Self {
        Self { view }
    }
}

impl Widget for BrowserPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let v = self.view;
        let arrow = if v.ascending { '▲' } else { '▼' };
        let mut block =
            Block::bordered().title(format!(" BROWSER  {}  by {} {arrow} ", v.status, v.sort));
        // A lit border is how the panel says the keyboard is its own.
        if v.active {
            block = block.border_style(Style::new().fg(theme::colour(theme::BROWSER_BORDER)));
        }
        let inner = block.inner(area);
        block.render(area, buf);
        if inner.height == 0 || inner.width < 12 {
            return;
        }

        // The query line sits at the bottom, with the commands under it while the mode is
        // active, so the list gets whatever is left above them.
        let searching = v.search.is_some();
        let rows_below = searching as u16 + v.active as u16;
        let list_height = inner.height.saturating_sub(rows_below) as usize;
        if v.active {
            buf.set_stringn(
                inner.x,
                inner.bottom() - 1,
                HINT,
                inner.width as usize,
                Style::new().add_modifier(Modifier::DIM),
            );
        }
        if searching {
            let query = v.search.as_deref().unwrap_or_default();
            buf.set_stringn(
                inner.x,
                inner.bottom() - 1 - v.active as u16,
                format!("/{query}"),
                inner.width as usize,
                Style::new().add_modifier(Modifier::BOLD),
            );
        }
        if list_height == 0 {
            return;
        }
        if v.rows.is_empty() {
            buf.set_stringn(
                inner.x,
                inner.y,
                &v.status,
                inner.width as usize,
                Style::new().add_modifier(Modifier::DIM),
            );
            return;
        }

        // Keep the selection on screen, with the list scrolling under it.
        let selected = v.selected.min(v.rows.len() - 1);
        let first = selected.saturating_sub(list_height.saturating_sub(1) / 2);
        let first = first.min(v.rows.len().saturating_sub(list_height));

        let width = inner.width as usize;
        let columns = BPM_WIDTH + KEY_WIDTH + LEN_WIDTH + GAP * 3;
        let name_width = width.saturating_sub(columns).max(8);
        for (line, row) in v.rows[first..].iter().take(list_height).enumerate() {
            let y = inner.y + line as u16;
            let index = first + line;
            let mut style = Style::new();
            if !row.analysed {
                style = style.add_modifier(Modifier::DIM);
            }
            if index == selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            let bpm = row
                .bpm
                .map(|b| format!("{b:.1}"))
                .unwrap_or_else(|| "-".into());
            let length = row.duration_secs.map(mmss).unwrap_or_else(|| "-".into());
            let name: String = row.name.chars().take(name_width).collect();
            let line_text = format!(
                "{name:<name_width$}{:gap$}{bpm:>BPM_WIDTH$}{:gap$}{:KEY_WIDTH$}{:gap$}{length:>LEN_WIDTH$}",
                "",
                "",
                "",
                "",
                gap = GAP,
            );
            buf.set_stringn(inner.x, y, &line_text, width, style);

            // The key goes on afterwards, so a compatible one can carry its own colour.
            let key_x = inner.x + (name_width + GAP + BPM_WIDTH + GAP) as u16;
            if let Some(key) = &row.key {
                let mut key_style = style;
                if row.compatible {
                    key_style = key_style
                        .fg(theme::colour(theme::KEY_MATCH))
                        .add_modifier(Modifier::BOLD);
                }
                if key_x < inner.right() {
                    buf.set_stringn(key_x, y, key, KEY_WIDTH, key_style);
                }
            }
        }
    }
}
