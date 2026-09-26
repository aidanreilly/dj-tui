use ratatui::layout::{Constraint, Layout, Rect};

/// Rows for one deck panel: borders, title, eight waveform rows, marker row, status row.
pub const DECK_HEIGHT: u16 = 13;
pub const PHASE_HEIGHT: u16 = 1;
pub const MIXER_WIDTH: u16 = 26;
/// Height of the collapsed horizontal mixer used on narrow terminals.
pub const MIXER_BAR_HEIGHT: u16 = 4;
/// Below this width the mixer strip collapses into a bar under the decks.
pub const WIDE_MIN_WIDTH: u16 = 110;

#[derive(Debug, Clone, Copy)]
pub struct ScreenLayout {
    pub deck_a: Rect,
    pub phase: Rect,
    pub deck_b: Rect,
    pub mixer: Rect,
    pub browser: Rect,
}

pub fn screen_layout(area: Rect) -> ScreenLayout {
    let decks_height = DECK_HEIGHT * 2 + PHASE_HEIGHT;
    let wide = area.width >= WIDE_MIN_WIDTH;

    let (top, mixer, browser) = if wide {
        let [top, browser] =
            Layout::vertical([Constraint::Length(decks_height), Constraint::Min(0)]).areas(area);
        let [decks, mixer] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(MIXER_WIDTH)]).areas(top);
        (decks, mixer, browser)
    } else {
        let [decks, mixer, browser] = Layout::vertical([
            Constraint::Length(decks_height),
            Constraint::Length(MIXER_BAR_HEIGHT),
            Constraint::Min(0),
        ])
        .areas(area);
        (decks, mixer, browser)
    };

    let [deck_a, phase, deck_b] = Layout::vertical([
        Constraint::Length(DECK_HEIGHT),
        Constraint::Length(PHASE_HEIGHT),
        Constraint::Length(DECK_HEIGHT),
    ])
    .areas(top);

    ScreenLayout { deck_a, phase, deck_b, mixer, browser }
}
