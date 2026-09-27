use crate::{screen_layout, DeckPanel, DeckView};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

#[derive(Debug, Clone, Default)]
pub struct MixerView {
    /// -1 is fully deck A, 1 is fully deck B.
    pub crossfader: f32,
    pub faders: [f32; 2],
    pub headphone_cue: [bool; 2],
    pub strips: [StripView; 2],
    /// Master bus peak, linear.
    pub master_meter: f32,
}

/// One channel's controls above the fader.
#[derive(Debug, Clone, Default)]
pub struct StripView {
    pub trim_db: f32,
    /// Indexed low, mid, high.
    pub eq_db: [f32; 3],
    /// Indexed low, mid, high.
    pub kills: [bool; 3],
    /// -1 full low-pass, 0 off, 1 full high-pass.
    pub filter: f32,
    /// Channel peak, linear, pre-fader.
    pub meter: f32,
    /// Name of the effect in the slot, empty when the strip has no slot.
    pub fx_name: &'static str,
    pub fx_on: bool,
    pub fx_wet: f32,
}

#[derive(Debug, Clone)]
pub struct ScreenView {
    pub decks: [DeckView; 2],
    pub mixer: MixerView,
    pub status: String,
    /// Latest event for the user (load results, errors). Shown above the status line.
    pub message: String,
    /// Deck B's beat phase relative to deck A, -0.5..0.5 beats; `None` without two grids.
    pub phase: Option<f64>,
    /// Draw the key list over everything else.
    pub help: bool,
}

/// What `?` puts on screen: the keys, in the order the spec's control table has them.
const HELP: &[(&str, &str)] = &[
    ("Space", "play or pause the focused deck"),
    ("c", "cue, held to preview"),
    ("1-8", "hot cue: trigger, or set an empty one"),
    ("Alt+1-8", "clear a hot cue"),
    ("Tab", "switch focused deck"),
    ("`", "send the next key to the other deck"),
    ("g then 0-9", "seek to a tenth of the track"),
    ("- / +", "tempo down and up, Alt for a fine step"),
    (", / .", "nudge back and forward"),
    ("s / q / k", "sync, quantize, key lock"),
    ("l", "loop four beats on and off"),
    ("i / I", "loop in and out by hand"),
    ("[ / ]", "halve and double the loop"),
    ("< / >", "beat jump back and forward"),
    ("f / F", "effect on and off, next effect"),
    ("9 / 0", "effect wet down and up"),
    ("p / P  d / D", "the effect's two knobs"),
    ("r / R", "trim"),
    ("t / T  y / Y  u / U", "EQ high, mid, low (Alt to kill)"),
    ("o / O", "filter toward low-pass and high-pass"),
    ("v / V", "channel fader"),
    ("m", "headphone cue"),
    ("arrows", "crossfader, Shift snaps to the end"),
    ("w", "waveform colour mode"),
    ("click", "seek on the waveform"),
    ("?", "this list"),
    ("Ctrl+Q", "quit"),
];

const XFADE_WIDTH: usize = 13;
const FADER_WIDTH: usize = 8;

fn dot(on: bool) -> char {
    if on {
        '●'
    } else {
        '○'
    }
}

fn crossfader_bar(x: f32) -> String {
    let pos = (((x.clamp(-1.0, 1.0) + 1.0) / 2.0) * (XFADE_WIDTH - 1) as f32).round() as usize;
    (0..XFADE_WIDTH)
        .map(|i| if i == pos { '╋' } else { '━' })
        .collect()
}

fn fader_bar(v: f32) -> String {
    let filled = (v.clamp(0.0, 1.0) * FADER_WIDTH as f32).round() as usize;
    (0..FADER_WIDTH)
        .map(|i| if i < filled { '█' } else { '▯' })
        .collect()
}

const BAR: usize = 7;
const METER_FLOOR_DB: f32 = -48.0;

fn level_bar(fraction: f32) -> String {
    let filled = (fraction.clamp(0.0, 1.0) * BAR as f32).round() as usize;
    (0..BAR)
        .map(|i| if i < filled { '█' } else { '▯' })
        .collect()
}

fn eq_cell(db: f32, kill: bool) -> String {
    if kill {
        format!("{:^w$}", "KILL", w = BAR)
    } else {
        level_bar((db + 26.0) / 32.0)
    }
}

fn filter_cell(v: f32) -> String {
    let pos = (((v.clamp(-1.0, 1.0) + 1.0) / 2.0) * (BAR - 1) as f32).round() as usize;
    (0..BAR)
        .map(|i| {
            if i == pos {
                if v.abs() < 0.05 {
                    '┼'
                } else {
                    '●'
                }
            } else {
                '─'
            }
        })
        .collect()
}

fn meter_spans(level: f32) -> Vec<Span<'static>> {
    let lit = if level <= 0.0 {
        0
    } else {
        let db = 20.0 * level.log10();
        (((db - METER_FLOOR_DB) / -METER_FLOOR_DB) * BAR as f32)
            .ceil()
            .clamp(0.0, BAR as f32) as usize
    };
    (0..BAR)
        .map(|i| {
            let color = match i {
                i if i + 1 == BAR => Color::Red,
                i if i + 3 >= BAR => Color::Yellow,
                _ => Color::Green,
            };
            if i < lit {
                Span::styled("▮", Style::new().fg(color))
            } else {
                Span::styled("▯", Style::new().add_modifier(Modifier::DIM))
            }
        })
        .collect()
}

fn row(label: &str, a: String, b: String) -> Line<'static> {
    Line::from(format!("{label:<5}{a} {b}"))
}

fn meter_row(label: &str, a: f32, b: f32) -> Line<'static> {
    let mut spans = vec![Span::raw(format!("{label:<5}"))];
    spans.extend(meter_spans(a));
    spans.push(Span::raw(" "));
    spans.extend(meter_spans(b));
    Line::from(spans)
}

/// The effect each channel is running, dimmed until it is switched on.
fn fx_row(a: &StripView, b: &StripView) -> Line<'static> {
    let cell = |s: &StripView| {
        let name: String = s.fx_name.chars().take(BAR).collect();
        let style = if s.fx_on {
            Style::new()
        } else {
            Style::new().add_modifier(Modifier::DIM)
        };
        Span::styled(format!("{name:^w$}", w = BAR), style)
    };
    Line::from(vec![
        Span::raw(format!("{:<5}", "FX")),
        cell(a),
        Span::raw(" "),
        cell(b),
    ])
}

fn render_mixer(f: &mut Frame, area: Rect, m: &MixerView) {
    let block = Block::bordered().title(" MIXER ");
    let inner = block.inner(area);
    let [a, b] = &m.strips;
    let lines = if inner.height >= 12 {
        vec![
            Line::from(format!("{:<5}{:^w$} {:^w$}", "", "A", "B", w = BAR)),
            row(
                "TRIM",
                level_bar((a.trim_db + 12.0) / 24.0),
                level_bar((b.trim_db + 12.0) / 24.0),
            ),
            row(
                "HI",
                eq_cell(a.eq_db[2], a.kills[2]),
                eq_cell(b.eq_db[2], b.kills[2]),
            ),
            row(
                "MID",
                eq_cell(a.eq_db[1], a.kills[1]),
                eq_cell(b.eq_db[1], b.kills[1]),
            ),
            row(
                "LOW",
                eq_cell(a.eq_db[0], a.kills[0]),
                eq_cell(b.eq_db[0], b.kills[0]),
            ),
            row("FLT", filter_cell(a.filter), filter_cell(b.filter)),
            Line::from(""),
            fx_row(a, b),
            row("WET", level_bar(a.fx_wet), level_bar(b.fx_wet)),
            Line::from(""),
            meter_row("PK", a.meter, b.meter),
            row("VOL", level_bar(m.faders[0]), level_bar(m.faders[1])),
            Line::from(format!(
                "{:<5}{:^w$} {:^w$}",
                "CUE",
                dot(m.headphone_cue[0]),
                dot(m.headphone_cue[1]),
                w = BAR
            )),
            Line::from(""),
            Line::from(format!("A {} B", crossfader_bar(m.crossfader))),
            {
                let mut spans = vec![Span::raw(format!("{:<5}", "MSTR"))];
                spans.extend(meter_spans(m.master_meter));
                Line::from(spans)
            },
        ]
    } else {
        let mut pk = vec![Span::raw("PK ")];
        pk.extend(meter_spans(a.meter));
        pk.push(Span::raw(" "));
        pk.extend(meter_spans(b.meter));
        vec![
            Line::from(format!(
                "VOL {} {}  CUE {}{}  A {} B",
                fader_bar(m.faders[0]),
                fader_bar(m.faders[1]),
                dot(m.headphone_cue[0]),
                dot(m.headphone_cue[1]),
                crossfader_bar(m.crossfader)
            )),
            Line::from(pk),
        ]
    };
    f.render_widget(Paragraph::new(lines).block(block), area);
}

const PHASE_WIDTH: usize = 33;

fn render_phase(f: &mut Frame, area: Rect, phase: Option<f64>) {
    let line = match phase {
        None => Line::from(Span::styled(
            " PHASE  no beat grid on both decks",
            Style::new().add_modifier(Modifier::DIM),
        )),
        Some(p) => {
            let centre = PHASE_WIDTH / 2;
            let pos = (((p.clamp(-0.5, 0.5) + 0.5) * (PHASE_WIDTH - 1) as f64).round()) as usize;
            let bar: String = (0..PHASE_WIDTH)
                .map(|i| {
                    if i == pos {
                        '█'
                    } else if i == centre {
                        '│'
                    } else {
                        '░'
                    }
                })
                .collect();
            let aligned = p.abs() < 0.02;
            let style = if aligned {
                Style::new().fg(Color::Green)
            } else {
                Style::new().fg(Color::Yellow)
            };
            Line::from(vec![
                Span::raw(" PHASE  B "),
                Span::styled(bar, style),
                Span::raw(format!(" {:+.2} beat", p)),
            ])
        }
    };
    f.render_widget(Paragraph::new(line), area);
}

pub fn render_screen(f: &mut Frame, v: &ScreenView) {
    let l = screen_layout(f.area());
    f.render_widget(DeckPanel::new(&v.decks[0]), l.deck_a);
    render_phase(f, l.phase, v.phase);
    f.render_widget(DeckPanel::new(&v.decks[1]), l.deck_b);
    render_mixer(f, l.mixer, &v.mixer);

    let browser = Block::bordered().title(" BROWSER ");
    let inner = browser.inner(l.browser);
    f.render_widget(browser, l.browser);
    f.render_widget(
        Paragraph::new("Library browser arrives in M8.")
            .style(Style::new().add_modifier(Modifier::DIM)),
        inner,
    );
    if inner.height > 0 {
        let status = Rect {
            y: inner.bottom() - 1,
            height: 1,
            ..inner
        };
        f.render_widget(
            Paragraph::new(v.status.as_str()).style(Style::new().add_modifier(Modifier::DIM)),
            status,
        );
    }
    if inner.height > 1 && !v.message.is_empty() {
        let message = Rect {
            y: inner.bottom() - 2,
            height: 1,
            ..inner
        };
        f.render_widget(Paragraph::new(v.message.as_str()), message);
    }
    if v.help {
        render_help(f);
    }
}

/// The key list, centred over whatever is behind it.
fn render_help(f: &mut Frame) {
    let area = f.area();
    let widest = HELP
        .iter()
        .map(|(keys, what)| keys.len() + what.len() + 3)
        .max()
        .unwrap_or(40);
    let width = (widest as u16 + 4).min(area.width);
    let height = (HELP.len() as u16 + 2).min(area.height);
    let box_area = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    let block = Block::bordered().title(" HELP ");
    let inner = block.inner(box_area);
    f.render_widget(Clear, box_area);
    f.render_widget(block, box_area);
    let key_width = HELP.iter().map(|(keys, _)| keys.len()).max().unwrap_or(0);
    let lines: Vec<Line> = HELP
        .iter()
        .take(inner.height as usize)
        .map(|(keys, what)| {
            Line::from(vec![
                Span::styled(
                    format!("{keys:<key_width$}  "),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
                Span::raw(*what),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}
