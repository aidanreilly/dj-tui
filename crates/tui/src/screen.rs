use crate::{screen_layout, BrowserPanel, BrowserView, DeckPanel, DeckView};
use engine::DeckId;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

#[derive(Debug, Clone)]
pub struct MixerView {
    /// -1 is fully deck A, 1 is fully deck B.
    pub crossfader: f32,
    pub faders: [f32; 2],
    pub headphone_cue: [bool; 2],
    /// Headphone blend: 0 is the cue bus alone, 1 is the master alone.
    pub cue_mix: f32,
    pub strips: [StripView; 2],
    /// Master bus peak, linear.
    pub master_meter: f32,
    /// How long an automated fade runs, in beats.
    pub fade_beats: f64,
    /// Where a running crossfader fade is heading, -1 to 1.
    pub crossfader_fade_target: Option<f32>,
    /// Which channel the deck keys act on. The other one is greyed out.
    pub focused: DeckId,
}

impl Default for MixerView {
    fn default() -> Self {
        Self {
            crossfader: 0.0,
            faders: [1.0; 2],
            headphone_cue: [false; 2],
            cue_mix: 0.5,
            strips: Default::default(),
            master_meter: 0.0,
            // Zero would draw "fade 0" on any view built without one.
            fade_beats: 8.0,
            crossfader_fade_target: None,
            focused: DeckId::A,
        }
    }
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
    /// Where a running fader fade is heading, 0 to 1.
    pub fade_target: Option<f32>,
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
    pub browser: BrowserView,
    /// The audio device screen, while it is open.
    pub devices: Option<DeviceView>,
}

/// The audio device chooser: what can be opened, and what is open now.
#[derive(Debug, Clone, Default)]
pub struct DeviceView {
    /// Name to open and what to show, in the order to list them.
    pub devices: Vec<(String, String)>,
    pub selected: usize,
    /// The device in use, marked in the list.
    pub current: String,
    /// What the device in use is doing: rate and period.
    pub note: String,
}

/// What `?` puts on screen: the keys, in the order the spec's control table has them.
const HELP: &[(&str, &str)] = &[
    ("Space", "play or pause the focused deck"),
    ("c", "cue, held to preview"),
    ("1-8", "hot cue: trigger, or set an empty one"),
    ("Alt+1-8", "clear a hot cue"),
    ("Tab", "switch focused deck; the other one greys out"),
    ("g then 0-9", "seek to a tenth of the track"),
    ("- / +", "ride both decks' tempo, Alt for a fine step"),
    (", / .", "the focused deck's pitch, Alt for a fine step"),
    ("< / >", "nudge back and forward"),
    ("s / q / k", "sync, quantize, key lock"),
    ("l", "loop four beats on and off"),
    ("i / I", "loop in and out by hand"),
    ("[ / ]", "halve and double the loop"),
    ("j / J", "beat jump back and forward"),
    ("f / F", "effect on and off, next effect"),
    ("9 / 0", "effect wet down and up"),
    ("p / P  d / D", "the effect's two knobs"),
    ("r / R", "trim"),
    ("t / y / u", "kill the highs, mids, lows"),
    ("T / Y / U", "give the highs, mids, lows to this deck"),
    ("o / O", "filter toward low-pass and high-pass"),
    ("v / V", "filter back to the middle, now or slowly"),
    ("m", "headphone cue"),
    ("h / H", "headphone mix, cue toward master"),
    ("\u{2190} \u{2192}", "crossfader, Shift goes hard to an end"),
    (
        "\u{2191} \u{2193}",
        "the focused deck's fader, Shift for an end",
    ),
    ("x / X", "crossfader to the middle, now or slowly"),
    ("Alt+arrows", "fade instead of stepping"),
    ("{ / }", "halve and double the fade length"),
    ("Esc", "stop every running fade"),
    ("w", "waveform colour mode"),
    ("click", "seek on the waveform"),
    ("b  /", "browser: b keeps the filter, / clears it"),
    ("Enter", "load the selection onto the focused deck"),
    ("Ctrl+D", "audio device"),
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

/// Which cell of a bar of `width` a -1..1 position lands in.
fn bipolar_cell(x: f32, width: usize) -> usize {
    (((x.clamp(-1.0, 1.0) + 1.0) / 2.0) * (width - 1) as f32).round() as usize
}

fn crossfader_bar(x: f32, fade_target: Option<f32>) -> String {
    let pos = bipolar_cell(x, XFADE_WIDTH);
    let target = fade_target.map(|t| bipolar_cell(t, XFADE_WIDTH));
    (0..XFADE_WIDTH)
        .map(|i| match i {
            i if i == pos => '╋',
            // A hollow marker at the far end of a running fade, so the move is visible.
            i if Some(i) == target => '╎',
            _ => '━',
        })
        .collect()
}

fn fader_bar(v: f32) -> String {
    let filled = (v.clamp(0.0, 1.0) * FADER_WIDTH as f32).round() as usize;
    (0..FADER_WIDTH)
        .map(|i| if i < filled { '█' } else { '▯' })
        .collect()
}

/// A level bar with a marker where a running fade is heading.
fn fading_level(v: f32, fade_target: Option<f32>) -> String {
    let bar = level_bar(v);
    let Some(target) = fade_target else {
        return bar;
    };
    let cell = (target.clamp(0.0, 1.0) * (BAR - 1) as f32).round() as usize;
    bar.chars()
        .enumerate()
        .map(|(i, c)| if i == cell { '╎' } else { c })
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

/// Styles for the two channel columns: the unfocused one is dimmed throughout the strip, so
/// which deck the keys act on reads at a glance.
fn column_styles(focused: DeckId) -> (Style, Style) {
    let dim = Style::new().add_modifier(Modifier::DIM);
    match focused {
        DeckId::A => (Style::new(), dim),
        DeckId::B => (dim, Style::new()),
    }
}

fn row(label: &str, a: String, b: String, focused: DeckId) -> Line<'static> {
    let (sa, sb) = column_styles(focused);
    Line::from(vec![
        Span::raw(format!("{label:<5}")),
        Span::styled(a, sa),
        Span::raw(" "),
        Span::styled(b, sb),
    ])
}

fn meter_row(label: &str, a: f32, b: f32, focused: DeckId) -> Line<'static> {
    let (sa, sb) = column_styles(focused);
    let mut spans = vec![Span::raw(format!("{label:<5}"))];
    spans.extend(meter_spans(a).into_iter().map(|s| s.patch_style(sa)));
    spans.push(Span::raw(" "));
    spans.extend(meter_spans(b).into_iter().map(|s| s.patch_style(sb)));
    Line::from(spans)
}

/// The effect each channel is running, dimmed until it is switched on.
fn fx_row(a: &StripView, b: &StripView, focused: DeckId) -> Line<'static> {
    let cell = |s: &StripView, column: Style| {
        let name: String = s.fx_name.chars().take(BAR).collect();
        let style = if s.fx_on {
            column
        } else {
            column.add_modifier(Modifier::DIM)
        };
        Span::styled(format!("{name:^w$}", w = BAR), style)
    };
    let (sa, sb) = column_styles(focused);
    Line::from(vec![
        Span::raw(format!("{:<5}", "FX")),
        cell(a, sa),
        Span::raw(" "),
        cell(b, sb),
    ])
}

fn render_mixer(f: &mut Frame, area: Rect, m: &MixerView) {
    let block = Block::bordered().title(" MIXER ");
    let inner = block.inner(area);
    let [a, b] = &m.strips;
    let lines = if inner.height >= 12 {
        vec![
            {
                let (sa, sb) = column_styles(m.focused);
                let mark = |d: DeckId, letter: &str| {
                    if m.focused == d {
                        format!("{:^w$}", format!("▸{letter}"), w = BAR)
                    } else {
                        format!("{letter:^w$}", w = BAR)
                    }
                };
                Line::from(vec![
                    Span::raw(format!("{:<5}", "")),
                    Span::styled(mark(DeckId::A, "A"), sa.add_modifier(Modifier::BOLD)),
                    Span::raw(" "),
                    Span::styled(mark(DeckId::B, "B"), sb.add_modifier(Modifier::BOLD)),
                ])
            },
            row(
                "TRIM",
                level_bar((a.trim_db + 12.0) / 24.0),
                level_bar((b.trim_db + 12.0) / 24.0),
                m.focused,
            ),
            row(
                "HI",
                eq_cell(a.eq_db[2], a.kills[2]),
                eq_cell(b.eq_db[2], b.kills[2]),
                m.focused,
            ),
            row(
                "MID",
                eq_cell(a.eq_db[1], a.kills[1]),
                eq_cell(b.eq_db[1], b.kills[1]),
                m.focused,
            ),
            row(
                "LOW",
                eq_cell(a.eq_db[0], a.kills[0]),
                eq_cell(b.eq_db[0], b.kills[0]),
                m.focused,
            ),
            row(
                "FLT",
                filter_cell(a.filter),
                filter_cell(b.filter),
                m.focused,
            ),
            Line::from(""),
            fx_row(a, b, m.focused),
            row("WET", level_bar(a.fx_wet), level_bar(b.fx_wet), m.focused),
            Line::from(""),
            meter_row("PK", a.meter, b.meter, m.focused),
            row(
                "VOL",
                fading_level(m.faders[0], a.fade_target),
                fading_level(m.faders[1], b.fade_target),
                m.focused,
            ),
            Line::from(format!(
                "{:<5}{:^w$} {:^w$}",
                "CUE",
                dot(m.headphone_cue[0]),
                dot(m.headphone_cue[1]),
                w = BAR
            )),
            row("MIX", level_bar(m.cue_mix), String::new(), m.focused),
            Line::from(""),
            Line::from(format!(
                "A {} B   fade {}",
                crossfader_bar(m.crossfader, m.crossfader_fade_target),
                m.fade_beats
            )),
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
                crossfader_bar(m.crossfader, m.crossfader_fade_target)
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
            " PHASE  needs both decks running with a beat grid",
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
    // Full screen gives the browser everything but the two lines at the foot.
    let browser_area = if v.browser.fullscreen {
        let area = f.area();
        Rect {
            height: area.height,
            ..area
        }
    } else {
        f.render_widget(DeckPanel::new(&v.decks[0]), l.deck_a);
        render_phase(f, l.phase, v.phase);
        f.render_widget(DeckPanel::new(&v.decks[1]), l.deck_b);
        render_mixer(f, l.mixer, &v.mixer);
        l.browser
    };
    let browser = Block::bordered();
    let inner = browser.inner(browser_area);
    let list_area = Rect {
        height: inner.height.saturating_sub(2),
        ..browser_area
    };
    f.render_widget(BrowserPanel::new(&v.browser), list_area);
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
    if let Some(devices) = &v.devices {
        render_devices(f, devices);
    }
    if v.help {
        render_help(f);
    }
}

/// The device chooser, centred over whatever is behind it.
fn render_devices(f: &mut Frame, v: &DeviceView) {
    let area = f.area();
    let widest = v
        .devices
        .iter()
        .map(|(name, description)| name.len() + description.len() + 6)
        .max()
        .unwrap_or(30)
        .max(30);
    let width = (widest as u16 + 4).min(area.width);
    let height = (v.devices.len().max(1) as u16 + 4).min(area.height);
    let box_area = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    let block = Block::bordered().title(" AUDIO DEVICE ");
    let inner = block.inner(box_area);
    f.render_widget(Clear, box_area);
    f.render_widget(block, box_area);
    if inner.height == 0 {
        return;
    }
    let mut lines: Vec<Line> = Vec::new();
    if v.devices.is_empty() {
        lines.push(Line::from("No audio devices found"));
    } else {
        let name_width = v.devices.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
        for (i, (name, description)) in v.devices.iter().enumerate() {
            let mark = if *name == v.current { '●' } else { ' ' };
            let mut style = Style::new();
            if i == v.selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            lines.push(Line::from(Span::styled(
                format!("{mark} {name:<name_width$}  {description}"),
                style,
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("in use: {} ({})", v.current, v.note),
        Style::new().add_modifier(Modifier::DIM),
    )));
    f.render_widget(Paragraph::new(lines), inner);
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
