use crate::mixer::{render_mixer, MixerView};
use crate::theme;
use crate::{screen_layout, BrowserPanel, BrowserView, DeckPanel, DeckView};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

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
    ("r / R", "trim"),
    ("t / y / u", "kill the lows, mids, highs on deck A"),
    ("T / Y / U", "kill the lows, mids, highs on deck B"),
    ("o / O", "master filter toward low-pass and high-pass"),
    ("v / V", "master filter back to the middle, now or slowly"),
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
    ("b", "browser, full screen; Esc comes back"),
    ("/", "browser beside the decks, with the filter cleared"),
    ("Enter", "load the selection onto the focused deck"),
    (
        "Alt+b",
        "in the browser: tempo window around the playing deck",
    ),
    ("Alt+k", "in the browser: only keys that would mix"),
    (
        "Alt+g",
        "in the browser: cycle the genres your library holds",
    ),
    (
        "Alt+d",
        "in the browser: ask Discogs the selected track's genre",
    ),
    ("Alt+a", "in the browser: read tags, look up, then analyse"),
    (
        "Alt+s / Alt+f",
        "in the browser: sort column, and panel size",
    ),
    ("bpm: key: genre:", "typed filters, e.g. bpm:124-128 key:9a"),
    ("Ctrl+D", "audio device"),
    ("?", "this list"),
    ("Ctrl+Q", "quit"),
];

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
                Style::new().fg(theme::colour(theme::GREEN))
            } else {
                Style::new().fg(theme::colour(theme::YELLOW))
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
