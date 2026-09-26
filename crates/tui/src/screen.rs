use crate::{screen_layout, DeckPanel, DeckView};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Paragraph},
    Frame,
};

#[derive(Debug, Clone)]
pub struct MixerView {
    /// -1 is fully deck A, 1 is fully deck B.
    pub crossfader: f32,
    pub faders: [f32; 2],
    pub headphone_cue: [bool; 2],
}

#[derive(Debug, Clone)]
pub struct ScreenView {
    pub decks: [DeckView; 2],
    pub mixer: MixerView,
    pub status: String,
}

const XFADE_WIDTH: usize = 13;
const FADER_WIDTH: usize = 8;

fn dot(on: bool) -> char {
    if on { '●' } else { '○' }
}

fn crossfader_bar(x: f32) -> String {
    let pos = (((x.clamp(-1.0, 1.0) + 1.0) / 2.0) * (XFADE_WIDTH - 1) as f32).round() as usize;
    (0..XFADE_WIDTH).map(|i| if i == pos { '╋' } else { '━' }).collect()
}

fn fader_bar(v: f32) -> String {
    let filled = (v.clamp(0.0, 1.0) * FADER_WIDTH as f32).round() as usize;
    (0..FADER_WIDTH).map(|i| if i < filled { '█' } else { '▯' }).collect()
}

fn render_mixer(f: &mut Frame, area: Rect, m: &MixerView) {
    let lines = vec![
        Line::from(format!("VOL A {}", fader_bar(m.faders[0]))),
        Line::from(format!("VOL B {}", fader_bar(m.faders[1]))),
        Line::from(format!("CUE {}   {} ", dot(m.headphone_cue[0]), dot(m.headphone_cue[1]))),
        Line::from(format!("A {} B", crossfader_bar(m.crossfader))),
    ];
    f.render_widget(Paragraph::new(lines).block(Block::bordered().title(" MIXER ")), area);
}

pub fn render_screen(f: &mut Frame, v: &ScreenView) {
    let l = screen_layout(f.area());
    f.render_widget(DeckPanel::new(&v.decks[0]), l.deck_a);
    f.render_widget(
        Paragraph::new(" PHASE ").style(Style::new().add_modifier(Modifier::DIM)),
        l.phase,
    );
    f.render_widget(DeckPanel::new(&v.decks[1]), l.deck_b);
    render_mixer(f, l.mixer, &v.mixer);

    let browser = Block::bordered().title(" BROWSER ");
    let inner = browser.inner(l.browser);
    f.render_widget(browser, l.browser);
    f.render_widget(
        Paragraph::new("Library browser arrives in M8.").style(Style::new().add_modifier(Modifier::DIM)),
        inner,
    );
    if inner.height > 0 {
        let status = Rect { y: inner.bottom() - 1, height: 1, ..inner };
        f.render_widget(Paragraph::new(v.status.as_str()), status);
    }
}
