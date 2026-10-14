//! Arming a plan and playing it out, one UI frame at a time.
//!
//! Before the start, the clock is the outgoing playhead: the mix waits for the downbeat it
//! was planned on however the tempo is ridden. From the start, and from the moment a brake
//! begins to wind down, it counts beats on the frame delta instead, since a loop wrapping or
//! a deck slowing would otherwise stall it or run it backwards.
//!
//! Nothing here touches the engine. `tick` hands back `Effect`s and `crate::app` carries them
//! out through the same paths the keys use, so `ControlState` stays the owner of every value.

use super::{plan, Context, Kind, Plan, Slot, StepAction};
use crate::automation::Curve;
use engine::DeckId;
use input::Control;

/// Two taps on one key closer together than this are a double tap.
pub const DOUBLE_TAP_SECS: f64 = 0.35;
/// The playhead landing this far past the start with the start not yet run means it jumped
/// there, by a hot cue or a seek, rather than playing there.
const JUMP_BEATS: f64 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Press {
    Armed {
        slot: usize,
        summary: String,
    },
    Disarmed,
    /// A double tap switched the slot off.
    Disabled,
    /// A double tap switched it back on.
    Enabled,
    Refused(String),
}

/// One thing for the app to do this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    /// Start an automated fade from wherever the control is now.
    Fade {
        control: Control,
        to: f32,
        secs: f64,
        curve: Curve,
    },
    Set {
        control: Control,
        to: f32,
    },
    /// Seek the deck if asked, set its rate if asked, and play it if it is not playing.
    Start {
        deck: DeckId,
        seek: Option<f64>,
        rate: Option<f64>,
    },
    Stop {
        deck: DeckId,
    },
    Loop {
        deck: DeckId,
        span: Option<(f64, f64)>,
    },
    KeyLock {
        deck: DeckId,
        on: bool,
    },
    /// Put a deck at a rate directly, beyond the tempo fader's range if need be.
    Rate {
        deck: DeckId,
        rate: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    Ready,
    /// Ready, but bent to fit the pair: see the plan's notes.
    Adapted,
    Unavailable,
    Disabled,
    Armed,
    Running,
}

/// One row of the transition list.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotLine {
    pub name: String,
    pub detail: String,
    pub state: SlotState,
}

#[derive(Debug, Clone)]
struct Ramp {
    deck: DeckId,
    from: f64,
    to: f64,
    beats: f64,
    done: f64,
}

#[derive(Debug, Clone)]
struct Live {
    slot: usize,
    plan: Plan,
    /// Index of the next step to run.
    next: usize,
    /// Beats from the start; negative before it.
    beats: f64,
    started: bool,
    /// The clock runs on the frame delta rather than the playhead.
    wall: bool,
    beat_secs: f64,
    /// Controls a hand has taken back.
    released: Vec<Control>,
    /// Each deck's rate when armed, for putting a brake back.
    rates: [f64; 2],
    ramps: Vec<Ramp>,
}

pub struct Transitions {
    slots: [Slot; 9],
    enabled: [bool; 9],
    last_tap: [Option<f64>; 9],
    live: Option<Live>,
    /// What was armed before the last tap armed something else, so the first tap of a
    /// double tap does not throw it away.
    previous: Option<Live>,
}

impl Transitions {
    pub fn new(slots: [Slot; 9]) -> Self {
        Self {
            slots,
            enabled: [true; 9],
            last_tap: [None; 9],
            live: None,
            previous: None,
        }
    }

    pub fn slots(&self) -> &[Slot; 9] {
        &self.slots
    }

    pub fn enabled(&self, slot: usize) -> bool {
        self.enabled[slot]
    }

    pub fn armed_slot(&self) -> Option<usize> {
        self.live.as_ref().map(|l| l.slot)
    }

    pub fn running(&self) -> bool {
        self.live.as_ref().is_some_and(|l| l.started)
    }

    /// The plan armed or running, if any.
    pub fn live_plan(&self) -> Option<&Plan> {
        self.live.as_ref().map(|l| &l.plan)
    }

    /// A tap on slot `slot` at `now` seconds.
    pub fn press(&mut self, slot: usize, now: f64, ctx: &Context) -> Press {
        let quick = self.last_tap[slot].is_some_and(|t| now - t <= DOUBLE_TAP_SECS);
        self.last_tap[slot] = Some(now);
        let name = self.slots[slot].kind.name();
        if !self.enabled[slot] {
            if quick {
                self.enabled[slot] = true;
                self.last_tap[slot] = None;
                return Press::Enabled;
            }
            return Press::Refused(format!(
                "{} ({name}) is off. A double tap switches it back on.",
                slot + 1
            ));
        }
        if quick {
            // The first tap armed this slot; take that back and put back what it replaced.
            if self
                .live
                .as_ref()
                .is_some_and(|l| l.slot == slot && !l.started)
            {
                self.live = self.previous.take();
            }
            self.enabled[slot] = false;
            self.last_tap[slot] = None;
            return Press::Disabled;
        }
        if let Some(live) = &self.live {
            if live.started {
                return Press::Refused(format!(
                    "{} is running. Esc stops it.",
                    live.plan.kind.name()
                ));
            }
            if live.slot == slot {
                self.live = None;
                self.previous = None;
                return Press::Disarmed;
            }
        }
        match plan(&self.slots[slot], ctx) {
            Err(why) => Press::Refused(why),
            Ok(plan) => {
                let od = &ctx.decks[plan.out_deck.index()];
                let summary = summary(&plan, od.position);
                let beat_secs = beat_secs(&plan, od.rate, ctx.sample_rate);
                let beats = (od.position - plan.start) / plan.beat_frames;
                self.previous = self.live.take();
                self.live = Some(Live {
                    slot,
                    plan,
                    next: 0,
                    beats,
                    started: false,
                    wall: false,
                    beat_secs,
                    released: Vec::new(),
                    rates: [ctx.decks[0].rate, ctx.decks[1].rate],
                    ramps: Vec::new(),
                });
                Press::Armed { slot, summary }
            }
        }
    }

    /// Stop whatever is armed or running, leaving every control where it is. True when
    /// there was something to stop.
    pub fn cancel(&mut self) -> bool {
        self.previous = None;
        self.live.take().is_some()
    }

    /// Drop the plan if it involves `deck`, for a track being replaced under it.
    pub fn cancel_deck(&mut self, deck: DeckId) -> bool {
        let hit = self
            .live
            .as_ref()
            .is_some_and(|l| l.plan.out_deck == deck || l.plan.in_deck == deck);
        if hit {
            self.cancel();
        }
        hit
    }

    /// A hand moved `control`: the running plan leaves it alone from now on.
    pub fn release(&mut self, control: Control) {
        if let Some(live) = &mut self.live {
            if !live.released.contains(&control) {
                live.released.push(control);
            }
        }
    }

    /// Advance by `dt` seconds with the decks as `ctx` has them, pushing what to do onto
    /// `out`. Returns a line worth showing when something finished or went wrong.
    pub fn tick(&mut self, dt: f64, ctx: &Context, out: &mut Vec<Effect>) -> Option<String> {
        let live = self.live.as_mut()?;
        let o = live.plan.out_deck.index();
        let od = &ctx.decks[o];
        if !live.wall {
            if !od.loaded {
                self.live = None;
                return Some("Transition off: the outgoing deck is empty".into());
            }
            let b = (od.position - live.plan.start) / live.plan.beat_frames;
            if !live.started && b > JUMP_BEATS && b - live.beats > JUMP_BEATS {
                let letter = super::letter(live.plan.out_deck);
                self.live = None;
                return Some(format!(
                    "Deck {letter} jumped past the mix point, so the transition is off"
                ));
            }
            live.beats = b;
        } else {
            live.beats += dt / live.beat_secs;
        }

        // Ramps first, so a step on the same beat (a rate put back) has the last word.
        let step_beats = if live.wall { dt / live.beat_secs } else { 0.0 };
        live.ramps.retain_mut(|r| {
            r.done += step_beats;
            let t = (r.done / r.beats).clamp(0.0, 1.0);
            out.push(Effect::Rate {
                deck: r.deck,
                rate: r.from + (r.to - r.from) * t,
            });
            t < 1.0
        });

        while live.next < live.plan.steps.len()
            && live.plan.steps[live.next].at <= live.beats + 1e-9
        {
            let step = live.plan.steps[live.next];
            live.next += 1;
            fire(live, step.action, ctx, out);
        }

        let done = live.next >= live.plan.steps.len()
            && live.ramps.is_empty()
            && live.beats >= live.plan.end_beats() - 1e-9;
        if done {
            let plan = self.live.take().map(|l| l.plan)?;
            return Some(format!(
                "{} done: deck {} is on",
                plan.kind.name(),
                super::letter(plan.in_deck)
            ));
        }
        None
    }

    /// What each key would do right now, for the list on screen.
    pub fn lines(&self, ctx: &Context) -> [SlotLine; 9] {
        std::array::from_fn(|s| self.line(s, ctx))
    }

    fn line(&self, s: usize, ctx: &Context) -> SlotLine {
        let slot = self.slots[s];
        let name = slot.kind.name().to_string();
        if !self.enabled[s] {
            return SlotLine {
                name,
                detail: "off".into(),
                state: SlotState::Disabled,
            };
        }
        if let Some(live) = self.live.as_ref().filter(|l| l.slot == s) {
            let total = (live.plan.end_beats() / 4.0).ceil().max(1.0) as u32;
            if live.started {
                let bar = ((live.beats / 4.0).floor() as u32 + 1).min(total);
                return SlotLine {
                    name,
                    detail: format!("▶ {bar}/{total}"),
                    state: SlotState::Running,
                };
            }
            let bars = (-live.beats / 4.0).ceil().max(1.0) as u32;
            let unit = if bars == 1 { "bar" } else { "bars" };
            return SlotLine {
                name,
                detail: format!("▸{bars:>2} {unit}"),
                state: SlotState::Armed,
            };
        }
        match plan(&slot, ctx) {
            Err(_) => SlotLine {
                name,
                detail: String::new(),
                state: SlotState::Unavailable,
            },
            Ok(p) => SlotLine {
                name,
                detail: if slot.kind == Kind::Auto {
                    auto_detail(&p)
                } else {
                    length(&p)
                },
                state: if p.notes.is_empty() {
                    SlotState::Ready
                } else {
                    SlotState::Adapted
                },
            },
        }
    }

    /// The few lines under the list: what is coming next and how it will go.
    pub fn next(&self, ctx: &Context) -> Vec<String> {
        if let Some(live) = &self.live {
            let p = &live.plan;
            let way = format!(
                "{}→{} {}",
                super::letter(p.out_deck),
                super::letter(p.in_deck),
                p.kind.name()
            );
            let mut lines = vec![way, format!("out {} · in {}", p.out_label, p.in_label)];
            lines.extend(p.notes.iter().cloned());
            return lines;
        }
        // With nothing armed, say what Auto would do, which is the best guess at what is next.
        let Some(auto) = (0..9).find(|&s| self.slots[s].kind == Kind::Auto && self.enabled[s])
        else {
            return Vec::new();
        };
        match plan(&self.slots[auto], ctx) {
            Err(why) => vec![why],
            Ok(p) => {
                let mut lines = vec![
                    format!("{} next: {}", auto + 1, p.kind.name()),
                    format!(
                        "{}→{} out {} · in {}",
                        super::letter(p.out_deck),
                        super::letter(p.in_deck),
                        p.out_label,
                        p.in_label
                    ),
                ];
                lines.extend(p.notes.iter().cloned());
                lines
            }
        }
    }
}

fn fire(live: &mut Live, action: StepAction, ctx: &Context, out: &mut Vec<Effect>) {
    let plan = &live.plan;
    let od = &ctx.decks[plan.out_deck.index()];
    let free = |c: &Control| !live.released.contains(c);
    match action {
        StepAction::Fade {
            control,
            to,
            beats,
            curve,
        } => {
            if free(&control) {
                out.push(Effect::Fade {
                    control,
                    to,
                    secs: beats * beat_secs(plan, od.rate, ctx.sample_rate),
                    curve,
                });
            }
        }
        StepAction::Set { control, to } => {
            if free(&control) {
                out.push(Effect::Set { control, to });
            }
        }
        StepAction::StartIncoming => {
            let id = &ctx.decks[plan.in_deck.index()];
            let sr = ctx.sample_rate as f64;
            // However late this frame is, the incoming track starts that late into itself, so
            // its downbeat still lines up with the one it was meant for.
            let over = live.beats.max(0.0);
            let in_rate = plan.in_rate.unwrap_or(id.rate);
            let seek = match plan.in_from {
                Some(from) => Some(
                    from + match plan.in_beat_frames {
                        Some(bf) if plan.gridded => over * bf,
                        _ => over * live.beat_secs * sr * in_rate,
                    },
                ),
                None => phase_fix(ctx, plan),
            };
            out.push(Effect::Start {
                deck: plan.in_deck,
                seek,
                rate: plan.in_rate,
            });
            live.beat_secs = beat_secs(plan, od.rate, ctx.sample_rate);
            live.started = true;
            live.wall = true;
        }
        StepAction::StopOutgoing => out.push(Effect::Stop {
            deck: plan.out_deck,
        }),
        StepAction::LoopOutgoing { beats } => out.push(Effect::Loop {
            deck: plan.out_deck,
            span: Some((plan.start, plan.start + beats * plan.beat_frames)),
        }),
        StepAction::EndLoopOutgoing => out.push(Effect::Loop {
            deck: plan.out_deck,
            span: None,
        }),
        StepAction::KeyLock { deck, on } => out.push(Effect::KeyLock { deck, on }),
        StepAction::RateRamp { deck, to, beats } => {
            live.beat_secs = beat_secs(plan, od.rate, ctx.sample_rate);
            live.wall = true;
            live.ramps.push(Ramp {
                deck,
                from: ctx.decks[deck.index()].rate,
                to,
                beats,
                done: 0.0,
            });
        }
        StepAction::RestoreRate { deck } => out.push(Effect::Rate {
            deck,
            rate: live.rates[deck.index()],
        }),
    }
}

/// An incoming deck that is already playing keeps its place, but its beats are pulled onto
/// the outgoing deck's, the way the second press of sync does.
fn phase_fix(ctx: &Context, plan: &Plan) -> Option<f64> {
    let sr = ctx.sample_rate as f64;
    let (od, id) = (
        &ctx.decks[plan.out_deck.index()],
        &ctx.decks[plan.in_deck.index()],
    );
    let (og, ig) = (od.grid?, id.grid?);
    let phase = |pos: f64, g: &analysis::tempo::BeatGrid| {
        ((pos / sr - g.first_beat_secs) / g.beat_secs()).rem_euclid(1.0)
    };
    let mut shift = phase(od.position, &og) - phase(id.position, &ig);
    if shift > 0.5 {
        shift -= 1.0;
    } else if shift < -0.5 {
        shift += 1.0;
    }
    Some((id.position + shift * ig.beat_secs() * sr).clamp(0.0, id.track_frames))
}

/// Seconds a beat of the outgoing deck lasts at `rate`.
fn beat_secs(plan: &Plan, rate: f64, sample_rate: u32) -> f64 {
    plan.beat_frames / sample_rate as f64 / rate.max(1e-3)
}

fn length(p: &Plan) -> String {
    let bars = (p.length_beats / 4.0).round() as u32;
    match bars {
        0 => "cut".into(),
        1 => "1 bar".into(),
        n => format!("{n} bars"),
    }
}

/// What Auto picked, short enough to sit beside its name.
fn auto_detail(p: &Plan) -> String {
    let short = match p.kind {
        Kind::BassSwap => "swap",
        Kind::LongBlend => "long",
        Kind::FilterBlend => "filt",
        Kind::JumpCut => "cut",
        Kind::DropSwap => "drop",
        Kind::LoopOut => "loop",
        Kind::TempoBridge => "brdg",
        Kind::Brake => "brake",
        Kind::Auto => "auto",
    };
    let bars = (p.length_beats / 4.0).round() as u32;
    if bars == 0 {
        format!("▹{short}")
    } else {
        format!("▹{short} {bars}")
    }
}

/// The message an armed plan gets.
fn summary(p: &Plan, position: f64) -> String {
    let bars = p.bars_until(position).ceil().max(1.0) as u32;
    let unit = if bars == 1 { "bar" } else { "bars" };
    let mut s = format!(
        "{} armed: {} to {} in {bars} {unit} (out: {}, in: {})",
        p.kind.name(),
        super::letter(p.out_deck),
        super::letter(p.in_deck),
        p.out_label,
        p.in_label
    );
    if !p.notes.is_empty() {
        s.push_str(", ");
        s.push_str(&p.notes.join(", "));
    }
    s
}
