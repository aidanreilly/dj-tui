//! The mixer strip: trim, EQ, faders, cue and the master controls.
//!
//! Every control here is drawn as a rail with a lit run on it rather than as a filled block.
//! Six block bars stacked up read as one slab, and a slab says nothing about where a control
//! is sitting. A run grows from the point the control is measured against: the centre for
//! anything with a neutral position, the left end for anything that runs up from nothing.

use crate::theme;
use engine::DeckId;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
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
    /// -1 full low-pass, 0 off, 1 full high-pass.
    pub filter: f32,
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
            filter: 0.0,
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
    /// Channel peak, linear, pre-fader.
    pub meter: f32,
    /// Where a running fader fade is heading, 0 to 1.
    pub fade_target: Option<f32>,
}

// --- The glyph family every control is drawn from ---

/// Unlit track: solid and light, so a control row reads as one continuous bar with a heavier
/// stretch lit on it. A dotted rail made the trim and EQ rows look like a different kind of
/// control from a fader, which they are not.
pub const RAIL: char = '─';
/// Lit run.
pub const RUN: char = '━';
/// Neutral point of a control that has one.
pub const TICK: char = '╋';
/// End of a run that grows from one end rather than from a centre.
pub const CAP: char = '╸';
/// Where a running fade is heading.
pub const TARGET: char = '╎';
/// A meter segment that is lit, and one that is not. Squares, because a meter reads as a
/// row of equal cells filling up rather than as a bar growing out of anything.
pub const LIT_PIP: char = '■';
pub const UNLIT_PIP: char = '□';

/// Width of one channel's cell, and of a meter.
pub const BAR: usize = 7;
const XFADE_WIDTH: usize = 13;
/// A master control reaches across both channel columns and the space between them, so it
/// reads as one control over the whole mix rather than sitting under either deck.
const MASTER_WIDTH: usize = 2 * BAR + 1;
const METER_FLOOR_DB: f32 = -48.0;
/// What the EQ rows show, which is less cut than the engine will take: past -26 dB the band
/// is gone and the extra travel tells you nothing.
const EQ_DISPLAY_FLOOR_DB: f32 = -26.0;
const EQ_CEILING_DB: f32 = 6.0;
const TRIM_RANGE_DB: f32 = 12.0;

fn dot(on: bool) -> char {
    if on {
        '●'
    } else {
        '○'
    }
}

/// A control with a neutral point: `v` is -1 to 1, and the run grows from the centre tick
/// toward wherever the control sits. Length reads as how far it has been moved and the side
/// reads as which way, so a glance gets both without a number.
pub fn centre_bar(v: f32, width: usize) -> String {
    let centre = width / 2;
    let reach = width - 1 - centre;
    let v = v.clamp(-1.0, 1.0);
    let lit = (v.abs() * reach as f32).round() as usize;
    (0..width)
        .map(|i| {
            if i == centre {
                return TICK;
            }
            let on_the_lit_side = if v > 0.0 { i > centre } else { i < centre };
            if v != 0.0 && on_the_lit_side && i.abs_diff(centre) <= lit {
                RUN
            } else {
                RAIL
            }
        })
        .collect()
}

/// One EQ band. Cut and boost get their own scales, because the band takes 26 dB of cut
/// against 6 dB of boost and one scale across the pair would leave 0 dB off centre, which
/// draws a flat EQ as a cut.
pub fn eq_bar(db: f32, kill: bool, width: usize) -> String {
    if kill {
        // `{:^}` pads the right with the odd column, which reads as pushed left on a row
        // whose neighbours are centred on a tick. Put it on the left instead.
        let spare = width.saturating_sub("KILL".len());
        let left = spare - spare / 2;
        return format!("{:left$}KILL{:right$}", "", "", right = spare / 2);
    }
    let v = if db >= 0.0 {
        db / EQ_CEILING_DB
    } else {
        db / -EQ_DISPLAY_FLOOR_DB
    };
    centre_bar(v, width)
}

/// A control that runs up from nothing: `v` is 0 to 1 and the run fills from the left,
/// ending in a cap so the value itself is one cell rather than the whole lit stretch.
pub fn fill_bar(v: f32, width: usize) -> String {
    let lit = (v.clamp(0.0, 1.0) * width as f32).round() as usize;
    (0..width)
        .map(|i| {
            if i + 1 == lit {
                CAP
            } else if i < lit {
                RUN
            } else {
                RAIL
            }
        })
        .collect()
}

/// Mark where a running fade is heading, so the move is visible before it arrives.
fn with_target(bar: String, cell: Option<usize>) -> String {
    let Some(cell) = cell else { return bar };
    bar.chars()
        .enumerate()
        .map(|(i, c)| if i == cell { TARGET } else { c })
        .collect()
}

fn fading_fill(v: f32, fade_target: Option<f32>, width: usize) -> String {
    let cell = fade_target.map(|t| (t.clamp(0.0, 1.0) * (width - 1) as f32).round() as usize);
    with_target(fill_bar(v, width), cell)
}

/// A control whose value is a place rather than an amount: an unbroken rail with one marker
/// on it, which is how the crossfader has always read.
pub fn position_bar(v: f32, width: usize) -> String {
    let at = (v.clamp(0.0, 1.0) * (width - 1) as f32).round() as usize;
    // The heavy rail the crossfader uses, since this is the same kind of reading.
    (0..width)
        .map(|i| if i == at { TICK } else { RUN })
        .collect()
}

/// Which cell of a bar of `width` a -1..1 position lands in.
fn bipolar_cell(x: f32, width: usize) -> usize {
    (((x.clamp(-1.0, 1.0) + 1.0) / 2.0) * (width - 1) as f32).round() as usize
}

/// The crossfader is a position between the decks rather than a quantity, so it keeps a
/// marker running along a lit rail instead of growing out of a centre.
fn crossfader_bar(x: f32, fade_target: Option<f32>) -> String {
    let pos = bipolar_cell(x, XFADE_WIDTH);
    let target = fade_target.map(|t| bipolar_cell(t, XFADE_WIDTH));
    (0..XFADE_WIDTH)
        .map(|i| match i {
            i if i == pos => TICK,
            i if Some(i) == target => TARGET,
            _ => RUN,
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
                i if i + 1 == BAR => theme::colour(theme::METER_CLIP),
                i if i + 3 >= BAR => theme::colour(theme::METER_HOT),
                _ => theme::colour(theme::METER_OK),
            };
            if i < lit {
                Span::styled(LIT_PIP.to_string(), Style::new().fg(color))
            } else {
                // Coloured rather than dimmed: a bare DIM leaves the unlit squares on the
                // terminal's own foreground, which fights the lit ones instead of sitting
                // behind them.
                Span::styled(
                    UNLIT_PIP.to_string(),
                    Style::new().fg(theme::colour(theme::BASE01)),
                )
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

/// The unlit rail sits back from the run, so a row of untouched controls recedes and the one
/// that has been moved is what the eye lands on.
fn rail_spans(bar: &str, style: Style) -> Vec<Span<'static>> {
    // Lit by what the character means, not by which character it is. A killed band's word
    // and a neutral tick are both state rather than a value being read off the row, so they
    // recede with the rail.
    spans_lit_by(bar, style, |c| matches!(c, RUN | CAP | TARGET))
}

/// A control whose value is one marker: the marker is what carries, and the rail it runs
/// along recedes. Colouring these by glyph put the rail in the foreground and the marker in
/// the muted grey, which is backwards.
fn marker_spans(bar: &str, style: Style) -> Vec<Span<'static>> {
    spans_lit_by(bar, style, |c| matches!(c, TICK | TARGET))
}

fn spans_lit_by(bar: &str, style: Style, lit: impl Fn(char) -> bool) -> Vec<Span<'static>> {
    // One span per character: a mixer row is at most 15 cells, so the cost is nothing and
    // the alternative is working out span boundaries by hand.
    bar.chars()
        .map(|c| {
            let s = match lit(c) {
                true => style,
                false => style.fg(theme::colour(theme::BASE01)),
            };
            Span::styled(c.to_string(), s)
        })
        .collect()
}

fn row(label: &str, a: String, b: String, focused: DeckId) -> Line<'static> {
    let (sa, sb) = column_styles(focused);
    let mut spans = vec![Span::raw(format!("{label:<5}"))];
    spans.extend(rail_spans(&a, sa));
    spans.push(Span::raw(" "));
    spans.extend(rail_spans(&b, sb));
    Line::from(spans)
}

fn master_row(label: &str, bar: String) -> Line<'static> {
    let mut spans = vec![Span::raw(format!("{label:<5}"))];
    spans.extend(rail_spans(&bar, Style::new()));
    Line::from(spans)
}

fn meter_row(label: &str, a: f32, b: f32, focused: DeckId) -> Line<'static> {
    let (sa, sb) = column_styles(focused);
    let mut spans = vec![Span::raw(format!("{label:<5}"))];
    spans.extend(meter_spans(a).into_iter().map(|s| s.patch_style(sa)));
    spans.push(Span::raw(" "));
    spans.extend(meter_spans(b).into_iter().map(|s| s.patch_style(sb)));
    Line::from(spans)
}

pub(crate) fn render_mixer(f: &mut Frame, area: Rect, m: &MixerView) {
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
                centre_bar(a.trim_db / TRIM_RANGE_DB, BAR),
                centre_bar(b.trim_db / TRIM_RANGE_DB, BAR),
                m.focused,
            ),
            row(
                "HI",
                eq_bar(a.eq_db[2], a.kills[2], BAR),
                eq_bar(b.eq_db[2], b.kills[2], BAR),
                m.focused,
            ),
            row(
                "MID",
                eq_bar(a.eq_db[1], a.kills[1], BAR),
                eq_bar(b.eq_db[1], b.kills[1], BAR),
                m.focused,
            ),
            row(
                "LOW",
                eq_bar(a.eq_db[0], a.kills[0], BAR),
                eq_bar(b.eq_db[0], b.kills[0], BAR),
                m.focused,
            ),
            master_row("FLT", centre_bar(m.filter, MASTER_WIDTH)),
            Line::from(""),
            meter_row("PK", a.meter, b.meter, m.focused),
            row(
                "VOL",
                fading_fill(m.faders[0], a.fade_target, BAR),
                fading_fill(m.faders[1], b.fade_target, BAR),
                m.focused,
            ),
            Line::from(format!(
                "{:<5}{:^w$} {:^w$}",
                "CUE",
                dot(m.headphone_cue[0]),
                dot(m.headphone_cue[1]),
                w = BAR
            )),
            row("MIX", fill_bar(m.cue_mix, BAR), String::new(), m.focused),
            Line::from(""),
            Line::from(format!(
                "A {} B",
                crossfader_bar(m.crossfader, m.crossfader_fade_target)
            )),
            // Its own labelled row, in the column every other label sits in. Riding on the
            // end of the crossfader row put it outside that column and ran it to the border.
            Line::from(format!("{:<5}{}", "FADE", m.fade_beats)),
            {
                let mut spans = vec![Span::raw(format!("{:<5}", "MSTR"))];
                spans.extend(meter_spans(m.master_meter));
                Line::from(spans)
            },
        ]
    } else {
        // The label column is the width the tall mixer uses, so a channel's fader and its
        // meter start in the same place and read as the one channel.
        let mut pk = vec![Span::raw(format!("{:<5}", "PK"))];
        pk.extend(meter_spans(a.meter));
        pk.push(Span::raw(" "));
        pk.extend(meter_spans(b.meter));
        let mut vol = vec![Span::raw(format!("{:<5}", "VOL"))];
        vol.extend(marker_spans(&position_bar(m.faders[0], BAR), Style::new()));
        vol.push(Span::raw(" "));
        vol.extend(marker_spans(&position_bar(m.faders[1], BAR), Style::new()));
        vol.push(Span::raw(format!(
            "  CUE {}{}  A {} B",
            dot(m.headphone_cue[0]),
            dot(m.headphone_cue[1]),
            crossfader_bar(m.crossfader, m.crossfader_fade_target)
        )));
        vec![Line::from(vol), Line::from(pk)]
    };
    f.render_widget(Paragraph::new(lines).block(block), area);
}
