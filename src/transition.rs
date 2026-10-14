//! Transitions: one key works out a whole mix from one track into the other.
//!
//! The planner here is pure. It takes what both decks are doing and returns a `Plan`: a list
//! of control moves timed in beats from the moment the mix starts, which is a downbeat on
//! the outgoing deck. `crate::app` arms a plan and plays it through `Automation`.
//!
//! Where a mix starts, in order of preference:
//! - a hot cue ahead in the back half of the outgoing track, which is a DJ's mark for it;
//! - a section change close ahead, from `analysis::phrase`;
//! - the next phrase line, every eight bars (four for a cut).
//!
//! Where the next track comes in: hot cue 1, else the main cue, else its first bar. A drop
//! swap goes for the drop instead. An incoming deck that is already playing stays where it
//! is, since someone put it there.
//!
//! Each kind is shaped by the pair it gets. A tempo gap the fader cannot close turns a blend
//! into a cut on the phrase line; clashing keys hand the mids over at once rather than layer
//! them; no grid means a fade timed by the clock; too little track left shortens the blend.

use crate::automation::{Curve, NO_GRID_BEAT_SECS};
use analysis::key::{Key, PitchClass};
use analysis::phrase::{Phrases, SectionKind};
use analysis::tempo::BeatGrid;
use engine::{DeckId, HOT_CUES};
use input::{Band, Control};

pub mod run;

/// EQ positions in the 0-to-1 `Control` space: 0.5 is flat and 0 is the floor, 26 dB down.
pub const EQ_FLAT: f32 = 0.5;
pub const EQ_CUT: f32 = 0.0;
/// Pulled back but still there, for a band that is on its way out.
const EQ_LOW: f32 = 0.3;

/// How far ahead a mix-out point is looked for.
const HORIZON_BARS: f64 = 32.0;
/// How close a section change has to be to win over the plain phrase line.
const SECTION_REACH_BARS: f64 = 16.0;
/// The shortest warning a mix gives before it starts, so the countdown can be seen.
const LEAD_BARS: f64 = 1.0;
const CUT_LEAD_BARS: f64 = 0.5;
/// Without a grid there are no bars, so a mix starts this soon.
const NO_GRID_LEAD_SECS: f64 = 1.0;
/// A tempo bridge rides the outgoing tempo over this many beats before the blend.
const BRIDGE_RIDE_BEATS: f64 = 32.0;
/// A brake winds down over this many beats into the downbeat.
const BRAKE_BEATS: f64 = 2.0;
/// Where a brake winds down to. Stopping dead holds one sample, which is a click.
const BRAKE_FLOOR_RATE: f64 = 0.08;
/// The bars a loop out holds on the outgoing deck.
const LOOP_OUT_BARS: f64 = 4.0;
/// Under this much track left after the next phrase line, auto loops the outgoing track.
const AUTO_LOOP_BARS: f64 = 16.0;
/// A long blend needs this much room or auto picks the shorter bass swap.
const AUTO_LONG_BARS: f64 = 48.0;
/// The fade a blend becomes when neither deck has a grid, in clock beats of half a second.
const NO_GRID_FADE_BEATS: f64 = 16.0;

/// What a slot does. The eight kinds plus `Auto`, which picks one of them to suit the pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    BassSwap,
    LongBlend,
    FilterBlend,
    JumpCut,
    DropSwap,
    LoopOut,
    TempoBridge,
    Brake,
    Auto,
}

impl Kind {
    pub const ALL: [Kind; 9] = [
        Kind::BassSwap,
        Kind::LongBlend,
        Kind::FilterBlend,
        Kind::JumpCut,
        Kind::DropSwap,
        Kind::LoopOut,
        Kind::TempoBridge,
        Kind::Brake,
        Kind::Auto,
    ];

    /// What the screen calls it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::BassSwap => "Bass swap",
            Kind::LongBlend => "Long blend",
            Kind::FilterBlend => "Filter blend",
            Kind::JumpCut => "Jump cut",
            Kind::DropSwap => "Drop swap",
            Kind::LoopOut => "Loop out",
            Kind::TempoBridge => "Tempo bridge",
            Kind::Brake => "Brake",
            Kind::Auto => "Auto",
        }
    }

    /// What the config file calls it.
    pub fn config_name(self) -> &'static str {
        match self {
            Kind::BassSwap => "bass-swap",
            Kind::LongBlend => "long-blend",
            Kind::FilterBlend => "filter-blend",
            Kind::JumpCut => "jump-cut",
            Kind::DropSwap => "drop-swap",
            Kind::LoopOut => "loop-out",
            Kind::TempoBridge => "tempo-bridge",
            Kind::Brake => "brake",
            Kind::Auto => "auto",
        }
    }

    pub fn parse(text: &str) -> Option<Kind> {
        Kind::ALL
            .into_iter()
            .find(|k| k.config_name() == text.trim().to_ascii_lowercase())
    }

    /// How long the kind runs when a slot does not say, in bars. A cut has no length.
    pub fn default_bars(self) -> u32 {
        match self {
            Kind::BassSwap | Kind::FilterBlend => 16,
            Kind::LongBlend => 32,
            Kind::LoopOut | Kind::TempoBridge => 8,
            Kind::JumpCut | Kind::DropSwap | Kind::Brake | Kind::Auto => 0,
        }
    }

    fn is_cut(self) -> bool {
        matches!(self, Kind::JumpCut | Kind::DropSwap | Kind::Brake)
    }
}

/// One key's worth of configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub kind: Kind,
    /// Length in bars, where the kind has one. `None` takes the kind's own.
    pub bars: Option<u32>,
}

impl Slot {
    pub fn new(kind: Kind) -> Self {
        Self { kind, bars: None }
    }
}

/// The row as it comes out of the box: the kinds in their own order, Auto on 9.
pub fn default_slots() -> [Slot; 9] {
    Kind::ALL.map(Slot::new)
}

/// What the planner needs to know about one deck.
#[derive(Debug, Clone, Copy)]
pub struct DeckInfo<'a> {
    pub loaded: bool,
    pub playing: bool,
    /// Playhead, in frames of the track.
    pub position: f64,
    pub track_frames: f64,
    pub rate: f64,
    pub key_lock: bool,
    pub grid: Option<BeatGrid>,
    /// Sections and the bar line. Empty when the track has no grid or no envelope.
    pub phrases: &'a Phrases,
    pub cue_point: f64,
    pub hot_cues: [Option<f64>; HOT_CUES],
    pub key: Option<Key>,
    /// Channel fader, 0 to 1.
    pub fader: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub decks: [DeckInfo<'a>; 2],
    /// -1 is deck A, 1 is deck B.
    pub crossfader: f32,
    pub sample_rate: u32,
    /// The tempo fader's reach either side of normal, as a share: 0.08 for ±8 %.
    pub tempo_range: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepAction {
    /// Ride a control to `to` over `beats`, starting from wherever it is when the step fires.
    Fade {
        control: Control,
        to: f32,
        beats: f64,
        curve: Curve,
    },
    /// Put a control somewhere at once.
    Set {
        control: Control,
        to: f32,
    },
    /// Seek the incoming deck to the plan's way in, set its rate and play it.
    StartIncoming,
    StopOutgoing,
    /// Loop the outgoing deck from where it is, for this many beats.
    LoopOutgoing {
        beats: f64,
    },
    EndLoopOutgoing,
    KeyLock {
        deck: DeckId,
        on: bool,
    },
    /// Wind a deck's rate to `to` over `beats`, beyond the tempo fader's range.
    RateRamp {
        deck: DeckId,
        to: f64,
        beats: f64,
    },
    /// Put a deck's rate back to what it was when the plan was armed.
    RestoreRate {
        deck: DeckId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    /// Beats from the start of the mix. Negative steps run ahead of it.
    pub at: f64,
    pub action: StepAction,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The kind that runs. For a slot set to Auto, the one it picked.
    pub kind: Kind,
    /// Picked by Auto.
    pub auto: bool,
    pub out_deck: DeckId,
    pub in_deck: DeckId,
    /// Frame of the outgoing track the mix starts on.
    pub start: f64,
    /// Frames of the outgoing track to a beat. Without a grid, half a second's worth.
    pub beat_frames: f64,
    /// The outgoing deck has a grid, so beats are beats.
    pub gridded: bool,
    pub length_beats: f64,
    /// Frame of the incoming track that plays at the start. `None` leaves it where it is.
    pub in_from: Option<f64>,
    /// Frames of the incoming track to a beat, to place it after a late start.
    pub in_beat_frames: Option<f64>,
    /// Rate the incoming deck starts at.
    pub in_rate: Option<f64>,
    pub steps: Vec<Step>,
    /// How the plan bent to fit, in a few words each: "keys clash", "no grid".
    pub notes: Vec<String>,
    /// What marks the start: "cue 3", "breakdown", "phrase".
    pub out_label: String,
    /// What marks the way in: "cue 1", "drop", "first bar".
    pub in_label: String,
}

impl Plan {
    /// Where the last step finishes, in beats from the start.
    pub fn end_beats(&self) -> f64 {
        self.steps
            .iter()
            .map(|s| match s.action {
                StepAction::Fade { beats, .. } | StepAction::RateRamp { beats, .. } => s.at + beats,
                _ => s.at,
            })
            .fold(0.0, f64::max)
    }

    /// Bars until the start, from the outgoing deck at `position`.
    pub fn bars_until(&self, position: f64) -> f64 {
        (self.start - position) / (self.beat_frames * 4.0)
    }
}

/// Work out what `slot` would do right now. `Err` says why it cannot.
pub fn plan(slot: &Slot, ctx: &Context) -> Result<Plan, String> {
    let (o, i) = sides(ctx)?;
    let mut kind = slot.kind;
    let auto = kind == Kind::Auto;
    if auto {
        kind = pick(ctx, o, i);
    }
    let bars = match slot.bars {
        Some(b) if !auto => b,
        _ => kind.default_bars(),
    };
    let mut p = Planner::new(ctx, o, i, kind, bars as f64 * 4.0);
    p.build();
    let mut plan = p.finish();
    plan.auto = auto;
    Ok(plan)
}

/// Which deck goes out and which comes in.
fn sides(ctx: &Context) -> Result<(usize, usize), String> {
    let playing: Vec<usize> = (0..2)
        .filter(|&d| ctx.decks[d].loaded && ctx.decks[d].playing)
        .collect();
    let out = match playing.as_slice() {
        [] => return Err("Nothing is playing to mix out of".into()),
        [one] => *one,
        _ => {
            if on_air(ctx, 1) > on_air(ctx, 0) {
                1
            } else {
                0
            }
        }
    };
    let inc = 1 - out;
    if !ctx.decks[inc].loaded {
        return Err(format!("Deck {} is empty", letter(deck_id(inc))));
    }
    Ok((out, inc))
}

/// How much of a deck reaches the master, roughly: its fader times its side of the
/// crossfader.
fn on_air(ctx: &Context, d: usize) -> f32 {
    let x = ctx.crossfader.clamp(-1.0, 1.0);
    let side = if d == 0 {
        (1.0 - x) / 2.0
    } else {
        (1.0 + x) / 2.0
    };
    ctx.decks[d].fader * side.min(0.5) * 2.0
}

/// Auto: the kind that suits this pair, in the order a DJ would think about it.
fn pick(ctx: &Context, o: usize, i: usize) -> Kind {
    let (od, id) = (&ctx.decks[o], &ctx.decks[i]);
    let (Some(_), Some(_)) = (od.grid, id.grid) else {
        // Nothing to beatmatch. A fade by the clock is the kindest thing to do.
        return Kind::LongBlend;
    };
    match tempo_fit(od, id, ctx.tempo_range) {
        Fit::Matched(_) => {}
        Fit::Gap(gap) if gap <= ctx.tempo_range * 2.0 => return Kind::TempoBridge,
        _ => return Kind::JumpCut,
    }
    let clock = Clock::of(od, ctx.sample_rate);
    let next = clock.next_line(od.position, LEAD_BARS, 8.0);
    let left = clock.bar_of(od.track_frames) - next;
    if left < AUTO_LOOP_BARS {
        return Kind::LoopOut;
    }
    let now = clock.bar_of(od.position);
    let breakdown_ahead = od.phrases.sections.iter().any(|s| {
        s.kind == SectionKind::Breakdown
            && (s.start_bar as f64) > now
            && (s.start_bar as f64) <= now + SECTION_REACH_BARS
    });
    if breakdown_ahead && id.phrases.first_drop().is_some() {
        return Kind::DropSwap;
    }
    let rate = match tempo_fit(od, id, ctx.tempo_range) {
        Fit::Matched(r) => r,
        _ => 1.0,
    };
    if keys_clash(od, id, rate) {
        return Kind::BassSwap;
    }
    if left >= AUTO_LONG_BARS {
        Kind::LongBlend
    } else {
        Kind::BassSwap
    }
}

enum Fit {
    /// The incoming deck can play at this rate and match.
    Matched(f64),
    /// The incoming deck would need this share more than its fader reaches.
    Gap(f64),
    NoGrid,
}

/// Tempo of the outgoing deck as it plays, and the rate the incoming one needs to meet it,
/// with half and double time folded.
fn tempo_fit(o: &DeckInfo, i: &DeckInfo, range: f64) -> Fit {
    let (Some(og), Some(ig)) = (o.grid, i.grid) else {
        return Fit::NoGrid;
    };
    let target = og.bpm * o.rate;
    let rate = [0.5, 1.0, 2.0]
        .map(|m| target / (ig.bpm * m))
        .into_iter()
        .min_by(|a, b| a.ln().abs().total_cmp(&b.ln().abs()))
        .unwrap_or(1.0);
    let off = (rate - 1.0).abs();
    if off <= range + 1e-9 {
        Fit::Matched(rate)
    } else {
        Fit::Gap(off)
    }
}

/// The key a deck sounds in at `rate`: moved by the pitch, unless key lock holds it.
fn sounding_key(d: &DeckInfo, rate: f64) -> Option<Key> {
    let key = d.key?;
    if d.key_lock {
        return Some(key);
    }
    let semis = (12.0 * rate.log2()).round() as i32;
    Some(Key {
        tonic: PitchClass(((key.tonic.0 as i32 + semis).rem_euclid(12)) as u8),
        mode: key.mode,
    })
}

/// Two keys sit together on the Camelot wheel: the same code, a step round it, or the
/// relative major or minor. An unknown key is given the benefit of the doubt.
fn keys_clash(o: &DeckInfo, i: &DeckInfo, in_rate: f64) -> bool {
    let (Some(a), Some(b)) = (sounding_key(o, o.rate), sounding_key(i, in_rate)) else {
        return false;
    };
    let code = |k: Key| {
        let c = k.camelot();
        let (n, l) = c.split_at(c.len() - 1);
        (n.parse::<i32>().unwrap_or(0), l.to_string())
    };
    let ((na, la), (nb, lb)) = (code(a), code(b));
    let step = (na - nb).rem_euclid(12);
    let near = step == 0 || step == 1 || step == 11;
    !((la == lb && near) || (step == 0 && la != lb))
}

/// The outgoing deck's bars, in frames of its track.
#[derive(Debug, Clone, Copy)]
struct Clock {
    beat_frames: f64,
    bar0: f64,
    gridded: bool,
}

impl Clock {
    fn of(d: &DeckInfo, sr: u32) -> Self {
        let sr = sr as f64;
        match d.grid {
            Some(g) => Self {
                beat_frames: g.beat_secs() * sr,
                bar0: d.phrases.bar_secs(&g, 0) * sr,
                gridded: true,
            },
            None => Self {
                beat_frames: NO_GRID_BEAT_SECS * sr,
                bar0: 0.0,
                gridded: false,
            },
        }
    }

    fn bar_of(&self, frame: f64) -> f64 {
        (frame - self.bar0) / (self.beat_frames * 4.0)
    }

    fn frame_of(&self, bar: f64) -> f64 {
        self.bar0 + bar * self.beat_frames * 4.0
    }

    /// The first bar line on a multiple of `every` bars at least `lead` bars ahead.
    fn next_line(&self, position: f64, lead: f64, every: f64) -> f64 {
        ((self.bar_of(position) + lead - 1e-9) / every).ceil() * every
    }
}

/// How the volume moves from one deck to the other.
#[derive(Debug, Clone, Copy)]
enum Volume {
    /// Ride the two channel faders, ending the incoming one where the outgoing one was.
    Faders { level: f32 },
    /// The crossfader is parked on the outgoing side, so ride it across to the mirror.
    Crossfader { to: f32 },
}

struct Planner<'c, 'a> {
    ctx: &'c Context<'a>,
    o: usize,
    i: usize,
    kind: Kind,
    beats: f64,
    clock: Clock,
    volume: Volume,
    steps: Vec<Step>,
    notes: Vec<String>,
    start: f64,
    out_label: String,
    in_from: Option<f64>,
    in_label: String,
    in_rate: Option<f64>,
}

impl<'c, 'a> Planner<'c, 'a> {
    fn new(ctx: &'c Context<'a>, o: usize, i: usize, kind: Kind, beats: f64) -> Self {
        let od = &ctx.decks[o];
        let x = ctx.crossfader;
        let parked = if o == 0 { x <= -0.5 } else { x >= 0.5 };
        let volume = if parked {
            Volume::Crossfader {
                to: (1.0 - (x + 1.0) / 2.0).clamp(0.0, 1.0),
            }
        } else {
            Volume::Faders {
                level: if od.fader > 0.0 { od.fader } else { 1.0 },
            }
        };
        Self {
            ctx,
            o,
            i,
            kind,
            beats,
            clock: Clock::of(od, ctx.sample_rate),
            volume,
            steps: Vec::new(),
            notes: Vec::new(),
            start: od.position,
            out_label: String::new(),
            in_from: None,
            in_label: String::new(),
            in_rate: None,
        }
    }

    fn od(&self) -> &DeckInfo<'a> {
        &self.ctx.decks[self.o]
    }

    fn id(&self) -> &DeckInfo<'a> {
        &self.ctx.decks[self.i]
    }

    fn out(&self) -> DeckId {
        deck_id(self.o)
    }

    fn inc(&self) -> DeckId {
        deck_id(self.i)
    }

    fn note(&mut self, text: &str) {
        if !self.notes.iter().any(|n| n == text) {
            self.notes.push(text.into());
        }
    }

    fn at(&mut self, at: f64, action: StepAction) {
        self.steps.push(Step { at, action });
    }

    fn set(&mut self, at: f64, control: Control, to: f32) {
        self.at(at, StepAction::Set { control, to });
    }

    fn fade(&mut self, at: f64, control: Control, to: f32, beats: f64, curve: Curve) {
        self.at(
            at,
            StepAction::Fade {
                control,
                to,
                beats,
                curve,
            },
        );
    }

    fn eq(&mut self, at: f64, deck: DeckId, band: Band, to: f32) {
        self.set(at, Control::Eq(deck, band), to);
    }

    fn eq_fade(&mut self, at: f64, deck: DeckId, band: Band, to: f32, beats: f64) {
        self.fade(at, Control::Eq(deck, band), to, beats, Curve::Position);
    }

    fn build(&mut self) {
        let od = *self.od();
        let id = *self.id();
        let both_grids = od.grid.is_some() && id.grid.is_some();
        let fit = tempo_fit(&od, &id, self.ctx.tempo_range);
        if let Fit::Matched(r) = fit {
            if (r - id.rate).abs() > 1e-9 {
                self.in_rate = Some(r);
            }
        }
        self.way_in();

        // Bend the kind to the pair before placing it.
        let mut kind = self.kind;
        let blend = !kind.is_cut();
        if !self.clock.gridded || !both_grids {
            if blend {
                self.note("no grid, fading by the clock");
                self.beats = NO_GRID_FADE_BEATS;
                kind = Kind::LongBlend;
            }
        } else if let Fit::Gap(_) = fit {
            match kind {
                Kind::TempoBridge => {}
                k if k.is_cut() => {}
                _ => {
                    self.note("tempo gap, cutting instead");
                    kind = Kind::JumpCut;
                }
            }
        } else if kind == Kind::TempoBridge {
            self.note("tempos already meet");
            kind = Kind::BassSwap;
        }
        let rate = self.in_rate.unwrap_or(id.rate);
        let clash = keys_clash(&od, &id, rate);
        if clash && blend && kind != Kind::JumpCut {
            self.note("keys clash");
        }

        self.place(kind);
        if (!self.clock.gridded || !both_grids) && blend {
            self.fade_by_clock();
            return;
        }
        match kind {
            Kind::BassSwap => self.bass_swap(clash),
            Kind::LongBlend => self.long_blend(clash),
            Kind::FilterBlend => self.filter_blend(),
            Kind::JumpCut => self.jump_cut(),
            Kind::DropSwap => self.drop_swap(),
            Kind::LoopOut => self.loop_out(),
            Kind::TempoBridge => self.tempo_bridge(clash),
            Kind::Brake => self.brake(),
            Kind::Auto => self.bass_swap(clash),
        }
        self.kind = kind;
    }

    /// Where the incoming track comes in.
    fn way_in(&mut self) {
        let id = *self.id();
        let sr = self.ctx.sample_rate as f64;
        if id.playing {
            self.in_from = None;
            self.in_label = "where it is".into();
            return;
        }
        if self.kind == Kind::DropSwap {
            if let (Some(g), Some(bar)) = (id.grid, id.phrases.first_drop()) {
                self.in_from = Some(id.phrases.bar_secs(&g, bar) * sr);
                self.in_label = "drop".into();
                return;
            }
        }
        if let Some(cue) = id.hot_cues[0] {
            self.in_from = Some(cue);
            self.in_label = "cue 1".into();
        } else if id.cue_point > 0.0 {
            self.in_from = Some(id.cue_point);
            self.in_label = "cue".into();
        } else if let Some(g) = id.grid {
            self.in_from = Some(id.phrases.bar_secs(&g, 0) * sr);
            self.in_label = "first bar".into();
        } else {
            self.in_from = Some(0.0);
            self.in_label = "start".into();
        }
    }

    /// Choose the start, and shorten the mix if the track runs out first.
    fn place(&mut self, kind: Kind) {
        let od = *self.od();
        let c = self.clock;
        let sr = self.ctx.sample_rate as f64;
        if !c.gridded {
            self.start = od.position + NO_GRID_LEAD_SECS * sr * od.rate;
            self.out_label = "now".into();
            return;
        }
        let end_bar = c.bar_of(od.track_frames);
        let now = c.bar_of(od.position);
        // Steps that run ahead of the start need that much warning on top of the lead.
        let ahead = match kind {
            Kind::TempoBridge => BRIDGE_RIDE_BEATS / 4.0,
            Kind::Brake => BRAKE_BEATS / 4.0,
            _ => 0.0,
        };
        let lead = if kind.is_cut() {
            CUT_LEAD_BARS
        } else {
            LEAD_BARS
        } + ahead;
        let earliest = now + lead;
        // A loop out makes its own runway.
        let needs = match kind {
            Kind::LoopOut => LOOP_OUT_BARS,
            Kind::TempoBridge => self.beats / 4.0,
            k if k.is_cut() => 0.0,
            _ => self.beats / 4.0,
        };
        let fits = |bar: f64| bar >= earliest - 1e-9 && bar + needs <= end_bar + 1e-9;

        if kind == Kind::DropSwap {
            let drop = od.phrases.sections.iter().find(|s| {
                s.kind == SectionKind::Drop
                    && fits(s.start_bar as f64)
                    && (s.start_bar as f64) <= now + HORIZON_BARS
            });
            if let Some(s) = drop {
                self.start = c.frame_of(s.start_bar as f64);
                self.out_label = "drop".into();
                return;
            }
            self.note("no breakdown ahead, cutting on the phrase");
            let bar = c.next_line(od.position, lead, 16.0);
            if fits(bar) {
                self.start = c.frame_of(bar);
                self.out_label = "phrase".into();
                return;
            }
        }

        // A hot cue ahead in the back half of the track is a DJ's own mix-out mark.
        let half = od.track_frames / 2.0;
        let cue = od
            .hot_cues
            .iter()
            .enumerate()
            .filter_map(|(n, c)| c.map(|f| (n, f)))
            .filter(|&(_, f)| f >= half && fits(c.bar_of(f)) && c.bar_of(f) <= now + HORIZON_BARS)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((n, f)) = cue {
            self.start = f;
            self.out_label = format!("cue {}", n + 1);
            return;
        }

        let section = od.phrases.sections.iter().skip(1).find(|s| {
            let bar = s.start_bar as f64;
            fits(bar) && bar <= now + SECTION_REACH_BARS
        });
        if let Some(s) = section {
            self.start = c.frame_of(s.start_bar as f64);
            self.out_label = section_label(s.kind).into();
            return;
        }

        let every = if kind.is_cut() { 4.0 } else { 8.0 };
        let line = c.next_line(od.position, lead, every);
        if fits(line) {
            self.start = c.frame_of(line);
            self.out_label = "phrase".into();
            return;
        }

        // Not enough track left for the whole mix on a phrase line: go on the next bar and
        // fit the mix into what remains, in whole blocks of four bars where there is room.
        let bar = c.next_line(od.position, lead, 1.0);
        self.start = c.frame_of(bar);
        self.out_label = "bar".into();
        if needs > 0.0 && kind != Kind::LoopOut {
            let room = (end_bar - bar).floor().max(1.0);
            let bars = if room >= 4.0 {
                (room / 4.0).floor() * 4.0
            } else {
                room
            };
            self.beats = (bars * 4.0).min(self.beats);
            self.note("short runway, shortened");
        }
    }

    // The volume handover, whichever controls carry it.

    fn vol_prep(&mut self) {
        let inc = self.inc();
        match self.volume {
            Volume::Faders { .. } if !self.id().playing => self.set(-1.0, Control::Fader(inc), 0.0),
            Volume::Faders { .. } => {}
            Volume::Crossfader { .. } => {
                let level = self.od().fader;
                self.set(-1.0, Control::Fader(inc), level);
            }
        }
    }

    fn vol_in(&mut self, at: f64, beats: f64) {
        match self.volume {
            Volume::Faders { level } => {
                let inc = self.inc();
                self.fade(at, Control::Fader(inc), level, beats, Curve::EqualPower);
            }
            Volume::Crossfader { .. } => {
                self.fade(at, Control::Crossfader, 0.5, beats, Curve::Position)
            }
        }
    }

    fn vol_out(&mut self, at: f64, beats: f64) {
        match self.volume {
            Volume::Faders { .. } => {
                let out = self.out();
                self.fade(at, Control::Fader(out), 0.0, beats, Curve::EqualPower);
            }
            Volume::Crossfader { to } => {
                self.fade(at, Control::Crossfader, to, beats, Curve::Position)
            }
        }
    }

    fn vol_cut(&mut self, at: f64) {
        match self.volume {
            Volume::Faders { level } => {
                let (out, inc) = (self.out(), self.inc());
                self.set(at, Control::Fader(inc), level);
                self.set(at, Control::Fader(out), 0.0);
            }
            Volume::Crossfader { to } => self.set(at, Control::Crossfader, to),
        }
    }

    /// The incoming EQ it starts with, set just ahead of the start so nothing changes until
    /// the mix is close.
    fn prep_in(&mut self, low: f32, mid: f32, high: f32) {
        let inc = self.inc();
        self.eq(-1.0, inc, Band::Low, low);
        self.eq(-1.0, inc, Band::Mid, mid);
        self.eq(-1.0, inc, Band::High, high);
        self.vol_prep();
    }

    /// Stop the old track and leave both channels flat.
    fn finish_at(&mut self, at: f64) {
        self.at(at, StepAction::StopOutgoing);
        let (out, inc) = (self.out(), self.inc());
        for band in [Band::Low, Band::Mid, Band::High] {
            self.eq(at, out, band, EQ_FLAT);
            self.eq(at, inc, band, EQ_FLAT);
        }
    }

    fn swap_lows(&mut self, at: f64) {
        let (out, inc) = (self.out(), self.inc());
        self.eq(at, out, Band::Low, EQ_CUT);
        self.eq(at, inc, Band::Low, EQ_FLAT);
    }

    fn bass_swap(&mut self, clash: bool) {
        let l = self.beats;
        let out = self.out();
        let inc = self.inc();
        self.prep_in(EQ_CUT, if clash { EQ_LOW } else { EQ_FLAT }, EQ_FLAT);
        self.at(0.0, StepAction::StartIncoming);
        self.vol_in(0.0, l / 2.0);
        self.swap_lows(l / 2.0);
        if clash {
            self.eq(l / 2.0, out, Band::Mid, EQ_CUT);
            self.eq(l / 2.0, inc, Band::Mid, EQ_FLAT);
        } else {
            self.eq_fade(l / 2.0, out, Band::Mid, EQ_LOW, l / 4.0);
        }
        self.vol_out(l / 2.0, l / 2.0);
        self.finish_at(l);
    }

    fn long_blend(&mut self, clash: bool) {
        let l = self.beats;
        let (out, inc) = (self.out(), self.inc());
        self.prep_in(EQ_CUT, if clash { EQ_CUT } else { EQ_LOW }, EQ_FLAT);
        self.at(0.0, StepAction::StartIncoming);
        self.vol_in(0.0, l / 4.0);
        self.eq_fade(l / 4.0, out, Band::High, EQ_LOW, l / 4.0);
        if !clash {
            self.eq_fade(l / 4.0, inc, Band::Mid, EQ_FLAT, l / 4.0);
        }
        self.swap_lows(l / 2.0);
        if clash {
            self.eq(l / 2.0, out, Band::Mid, EQ_CUT);
            self.eq(l / 2.0, inc, Band::Mid, EQ_FLAT);
        } else {
            self.eq_fade(l / 2.0, out, Band::Mid, EQ_LOW, l / 4.0);
        }
        self.vol_out(l * 0.75, l / 4.0);
        self.finish_at(l);
    }

    /// No per-channel filter on the mixer, so the EQ plays the part: the incoming track
    /// arrives as highs alone and opens up, while the outgoing one closes down to its lows.
    fn filter_blend(&mut self) {
        let l = self.beats;
        let (out, inc) = (self.out(), self.inc());
        self.prep_in(EQ_CUT, EQ_CUT, EQ_FLAT);
        self.at(0.0, StepAction::StartIncoming);
        self.vol_in(0.0, l / 4.0);
        self.eq_fade(l / 4.0, inc, Band::Mid, EQ_FLAT, l / 2.0);
        self.eq_fade(l / 4.0, out, Band::High, EQ_CUT, l / 2.0);
        self.eq_fade(l / 2.0, out, Band::Mid, EQ_LOW, l / 4.0);
        self.swap_lows(l * 0.75);
        self.vol_out(l * 0.75, l / 4.0);
        self.finish_at(l);
    }

    fn jump_cut(&mut self) {
        self.beats = 0.0;
        self.prep_in(EQ_FLAT, EQ_FLAT, EQ_FLAT);
        self.at(0.0, StepAction::StartIncoming);
        self.vol_cut(0.0);
        self.finish_at(0.0);
    }

    fn drop_swap(&mut self) {
        self.beats = 0.0;
        let out = self.out();
        self.prep_in(EQ_FLAT, EQ_FLAT, EQ_FLAT);
        // Thin the top out over the last bar, so the drop that follows lands harder.
        self.eq_fade(-4.0, out, Band::High, EQ_LOW, 4.0);
        self.at(0.0, StepAction::StartIncoming);
        self.vol_cut(0.0);
        self.finish_at(0.0);
    }

    fn loop_out(&mut self) {
        let l = self.beats;
        self.prep_in(EQ_CUT, EQ_FLAT, EQ_FLAT);
        self.at(
            0.0,
            StepAction::LoopOutgoing {
                beats: (LOOP_OUT_BARS * 4.0).min(l.max(4.0)),
            },
        );
        self.at(0.0, StepAction::StartIncoming);
        self.vol_in(0.0, l / 2.0);
        self.swap_lows(l / 2.0);
        self.vol_out(l / 2.0, l / 2.0);
        self.at(l, StepAction::EndLoopOutgoing);
        self.finish_at(l);
    }

    /// Ride the outgoing tempo as far toward the incoming one as its fader goes, then blend.
    fn tempo_bridge(&mut self, clash: bool) {
        let od = *self.od();
        let id = *self.id();
        let range = self.ctx.tempo_range;
        let out = self.out();
        let (Some(og), Some(ig)) = (od.grid, id.grid) else {
            self.jump_cut();
            return;
        };
        // The incoming tempo, folded nearest the outgoing one.
        let target = og.bpm * od.rate;
        let in_bpm = [0.5, 1.0, 2.0]
            .map(|m| ig.bpm * m)
            .into_iter()
            .min_by(|a, b| (target / a).ln().abs().total_cmp(&(target / b).ln().abs()))
            .unwrap_or(ig.bpm);
        let out_rate = (in_bpm / og.bpm).clamp(1.0 - range, 1.0 + range);
        let in_rate = og.bpm * out_rate / in_bpm;
        if !od.key_lock {
            self.at(
                -BRIDGE_RIDE_BEATS,
                StepAction::KeyLock {
                    deck: out,
                    on: true,
                },
            );
        }
        let v = (((out_rate - 1.0) / range + 1.0) / 2.0) as f32;
        self.fade(
            -BRIDGE_RIDE_BEATS,
            Control::Tempo(out),
            v,
            BRIDGE_RIDE_BEATS,
            Curve::Position,
        );
        if (in_rate - 1.0).abs() <= range + 1e-9 {
            self.in_rate = Some(in_rate);
            self.bass_swap(clash);
        } else {
            self.note("still apart after the ride, cutting");
            self.in_rate = Some(in_rate.clamp(1.0 - range, 1.0 + range));
            self.jump_cut();
        }
    }

    fn brake(&mut self) {
        self.beats = 0.0;
        let od = *self.od();
        let out = self.out();
        self.prep_in(EQ_FLAT, EQ_FLAT, EQ_FLAT);
        // The stretcher would hold the pitch up while the record slows, which is not a brake.
        if od.key_lock {
            self.at(
                -BRAKE_BEATS,
                StepAction::KeyLock {
                    deck: out,
                    on: false,
                },
            );
        }
        self.at(
            -BRAKE_BEATS,
            StepAction::RateRamp {
                deck: out,
                to: BRAKE_FLOOR_RATE,
                beats: BRAKE_BEATS,
            },
        );
        self.at(0.0, StepAction::StartIncoming);
        self.vol_cut(0.0);
        self.finish_at(0.0);
        self.at(0.0, StepAction::RestoreRate { deck: out });
        if od.key_lock {
            self.at(
                0.0,
                StepAction::KeyLock {
                    deck: out,
                    on: true,
                },
            );
        }
    }

    /// No grid on one side or the other, so no beats to line up: an equal-power fade timed
    /// by the clock, with the lows kept apart.
    fn fade_by_clock(&mut self) {
        let l = self.beats;
        self.prep_in(EQ_CUT, EQ_FLAT, EQ_FLAT);
        self.at(0.0, StepAction::StartIncoming);
        self.vol_in(0.0, l / 2.0);
        self.swap_lows(l / 2.0);
        self.vol_out(l / 2.0, l / 2.0);
        self.finish_at(l);
        self.kind = Kind::LongBlend;
    }

    fn finish(mut self) -> Plan {
        // Steps run in the order they fire; ties keep the order they were written in.
        self.steps.sort_by(|a, b| a.at.total_cmp(&b.at));
        let id = *self.id();
        let sr = self.ctx.sample_rate as f64;
        let mut plan = Plan {
            kind: self.kind,
            auto: false,
            out_deck: self.out(),
            in_deck: self.inc(),
            start: self.start,
            beat_frames: self.clock.beat_frames,
            gridded: self.clock.gridded,
            length_beats: self.beats,
            in_from: self.in_from,
            in_beat_frames: id.grid.map(|g| g.beat_secs() * sr),
            in_rate: self.in_rate,
            steps: self.steps,
            notes: self.notes,
            out_label: self.out_label,
            in_label: self.in_label,
        };
        plan.length_beats = plan.end_beats().max(0.0);
        plan
    }
}

fn section_label(kind: SectionKind) -> &'static str {
    match kind {
        SectionKind::Intro => "intro",
        SectionKind::Body => "body",
        SectionKind::Breakdown => "breakdown",
        SectionKind::Drop => "drop",
        SectionKind::Outro => "outro",
    }
}

fn deck_id(index: usize) -> DeckId {
    if index == 0 {
        DeckId::A
    } else {
        DeckId::B
    }
}

pub fn letter(d: DeckId) -> char {
    match d {
        DeckId::A => 'A',
        DeckId::B => 'B',
    }
}
