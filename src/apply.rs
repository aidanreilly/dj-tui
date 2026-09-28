//! Maps input actions onto engine commands.
//!
//! The UI owns the absolute value of every continuous control (faders, crossfader, tempo) in
//! `ControlState`, so several key presses between two audio callbacks each count.
//! Actions for features not built yet produce no command.

use engine::dsp::{EqBand, EQ_MAX_DB, TRIM_RANGE_DB};
use engine::fx::{FxKind, FX_PARAMS};
use engine::{Command, DeckId, Snapshot};
use input::{Action, Dir};
use midi::Control;

/// Step sizes and limits for keyboard controls.
#[derive(Debug, Clone)]
pub struct Controls {
    pub tempo_range: f64,
    /// Frames one nudge shifts the playhead by.
    pub nudge_frames: f64,
    pub tempo_step: f64,
    pub tempo_fine_step: f64,
    pub crossfader_step: f32,
}

impl Controls {
    /// `tempo_range_percent` is the configured fader range (8, 16 or 50), `sample_rate` the
    /// session rate, which is what a nudge in milliseconds has to be measured against.
    pub fn new(tempo_range_percent: u8, sample_rate: u32) -> Self {
        Self {
            tempo_range: tempo_range_percent as f64 / 100.0,
            nudge_frames: sample_rate as f64 * NUDGE_SECS,
            tempo_step: 0.005,
            tempo_fine_step: 0.0005,
            crossfader_step: 0.1,
        }
    }
}

/// The UI's authoritative copy of continuous control values.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlState {
    pub crossfader: f32,
    pub faders: [f32; 2],
    pub headphone_cue: [bool; 2],
    pub rates: [f64; 2],
    pub strips: [StripState; 2],
    /// Frames per beat from each deck's beat grid, `None` until a track is analysed.
    pub beat_frames: [Option<f64>; 2],
    /// Frame of the first beat, the anchor quantized positions snap to.
    pub first_beat_frames: [f64; 2],
    /// Length of the next loop, and of a beat jump, in beats.
    pub loop_beats: [f64; 2],
    /// Loop in point waiting for its out point, in frames.
    pub loop_in: [Option<f64>; 2],
    pub quantize: [bool; 2],
    pub key_lock: [bool; 2],
    /// Headphone blend: 0 is the cue bus alone, 1 is the master alone.
    pub cue_mix: f32,
    pub fx: [FxState; 2],
}

/// The effect slot for one channel, as the UI holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxState {
    pub kind: FxKind,
    pub on: bool,
    pub wet: f32,
    /// The two knobs, which keep their positions when the unit changes.
    pub params: [f32; FX_PARAMS],
}

impl Default for FxState {
    fn default() -> Self {
        Self {
            kind: FxKind::default(),
            on: false,
            wet: 0.0,
            params: [0.5; FX_PARAMS],
        }
    }
}

/// Trim, EQ and filter for one channel. Arrays are indexed by `EqBand as usize`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StripState {
    pub trim_db: f32,
    pub eq_db: [f32; 3],
    pub kills: [bool; 3],
    pub filter: f32,
}

pub const FADER_STEP_DB: f32 = 2.0;
/// Lowest fader position short of silence, and where the first step up from silence lands.
pub const FADER_FLOOR_DB: f32 = -40.0;
pub const TRIM_STEP_DB: f32 = 1.0;
pub const EQ_STEP_DB: f32 = 2.0;
/// Lowest EQ knob position short of a kill.
pub const EQ_FLOOR_DB: f32 = -26.0;
pub const FILTER_STEP: f32 = 0.1;
pub const FX_WET_STEP: f32 = 0.1;
pub const FX_PARAM_STEP: f32 = 0.05;
pub const CUE_MIX_STEP: f32 = 0.1;
/// How far one nudge moves the playhead. Small enough to beatmatch by ear.
pub const NUDGE_SECS: f64 = 0.01;
/// A bar in 4/4, the length a CDJ's loop key reaches for.
pub const DEFAULT_LOOP_BEATS: f64 = 4.0;
pub const MIN_LOOP_BEATS: f64 = 0.125;
pub const MAX_LOOP_BEATS: f64 = 32.0;

fn band(b: input::Band) -> EqBand {
    match b {
        input::Band::Low => EqBand::Low,
        input::Band::Mid => EqBand::Mid,
        input::Band::High => EqBand::High,
    }
}

pub fn db_to_amp(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Step a fader in dB. A linear step is 0.4 dB at the top of the travel and 26 dB at the
/// bottom, which is backwards for a fader: the ear hears decibels.
fn step_fader_db(v: f32, dir: Dir) -> f32 {
    let up = matches!(dir, Dir::Up);
    if v <= 0.0 {
        return if up { db_to_amp(FADER_FLOOR_DB) } else { 0.0 };
    }
    let db = 20.0 * v.log10() + FADER_STEP_DB * sign(dir);
    if db < FADER_FLOOR_DB {
        return if up { db_to_amp(FADER_FLOOR_DB) } else { 0.0 };
    }
    db_to_amp(db.min(0.0))
}

/// Step `v` and round to the step grid, so stepping back always lands exactly.
fn step(v: f32, by: f32, lo: f32, hi: f32) -> f32 {
    (((v + by) / by.abs()).round() * by.abs()).clamp(lo, hi)
}

impl Default for ControlState {
    fn default() -> Self {
        Self {
            crossfader: 0.0,
            faders: [1.0; 2],
            headphone_cue: [false; 2],
            rates: [1.0; 2],
            strips: Default::default(),
            beat_frames: [None; 2],
            first_beat_frames: [0.0; 2],
            loop_beats: [DEFAULT_LOOP_BEATS; 2],
            loop_in: [None; 2],
            quantize: [false; 2],
            key_lock: [false; 2],
            cue_mix: 0.5,
            fx: [FxState::default(); 2],
        }
    }
}

fn sign(d: Dir) -> f32 {
    match d {
        Dir::Up => 1.0,
        Dir::Down => -1.0,
    }
}

/// The playhead as the loop and jump keys see it: on the nearest beat when quantize is on.
fn from_here(st: &ControlState, snap: &Snapshot, i: usize) -> f64 {
    let pos = snap.decks[i].position;
    let Some(beat) = st.beat_frames[i] else {
        return pos;
    };
    if !st.quantize[i] {
        return pos;
    }
    let anchor = st.first_beat_frames[i];
    anchor + ((pos - anchor) / beat).round() * beat
}

/// Set the loop length in beats, halving or doubling within the range a CDJ offers.
fn scale_loop(st: &mut ControlState, snap: &Snapshot, d: DeckId, by: f64) -> Option<Command> {
    let i = d.index();
    let beats = (st.loop_beats[i] * by).clamp(MIN_LOOP_BEATS, MAX_LOOP_BEATS);
    st.loop_beats[i] = beats;
    // A running loop keeps its in point and changes length under the playhead.
    let (start, _) = snap.decks[i].loop_span?;
    let beat = st.beat_frames[i]?;
    Some(Command::SetLoop(d, Some((start, start + beats * beat))))
}

/// Put a control at an absolute position from 0 to 1, as a MIDI fader or knob does. The
/// stepped keyboard path cannot express this: the hardware already knows where it is pointing.
pub fn set_control(
    st: &mut ControlState,
    c: &Controls,
    control: Control,
    value: f32,
) -> Option<Command> {
    let v = value.clamp(0.0, 1.0);
    Some(match control {
        Control::Crossfader => {
            st.crossfader = v * 2.0 - 1.0;
            Command::SetCrossfader(st.crossfader)
        }
        Control::CueMix => {
            st.cue_mix = v;
            Command::SetCueMix(v)
        }
        Control::Fader(d) => {
            st.faders[d.index()] = v;
            Command::SetChannelFader(d, v)
        }
        Control::Tempo(d) => {
            let rate = 1.0 + (v as f64 * 2.0 - 1.0) * c.tempo_range;
            st.rates[d.index()] = rate;
            Command::SetRate(d, rate)
        }
        Control::Trim(d) => {
            let db = (v * 2.0 - 1.0) * TRIM_RANGE_DB;
            st.strips[d.index()].trim_db = db;
            Command::SetTrim(d, db)
        }
        Control::Eq(d, b) => {
            let b = band(b);
            // Below centre runs down to the kill floor, above it up to the maximum boost.
            let db = if v >= 0.5 {
                (v - 0.5) * 2.0 * EQ_MAX_DB
            } else {
                (0.5 - v) * 2.0 * EQ_FLOOR_DB
            };
            st.strips[d.index()].eq_db[b as usize] = db;
            Command::SetEq(d, b, db)
        }
        Control::Filter(d) => {
            let f = v * 2.0 - 1.0;
            st.strips[d.index()].filter = f;
            Command::SetFilter(d, f)
        }
        Control::FxWet(d) => {
            st.fx[d.index()].wet = v;
            Command::SetFxWet(d, v)
        }
        Control::FxParam(d, index) => {
            let knob = st.fx[d.index()].params.get_mut(index)?;
            *knob = v;
            Command::SetFxParam(d, index, v)
        }
    })
}

/// Where the UI has a control, from 0 to 1, which is what soft takeover compares against.
pub fn control_value(st: &ControlState, c: &Controls, control: Control) -> f32 {
    match control {
        Control::Crossfader => (st.crossfader + 1.0) / 2.0,
        Control::CueMix => st.cue_mix,
        Control::Fader(d) => st.faders[d.index()],
        Control::Tempo(d) => {
            (((st.rates[d.index()] - 1.0) / c.tempo_range + 1.0) / 2.0).clamp(0.0, 1.0) as f32
        }
        Control::Trim(d) => (st.strips[d.index()].trim_db / TRIM_RANGE_DB + 1.0) / 2.0,
        Control::Eq(d, b) => {
            let db = st.strips[d.index()].eq_db[band(b) as usize];
            if db >= 0.0 {
                0.5 + db / EQ_MAX_DB / 2.0
            } else {
                0.5 - db / EQ_FLOOR_DB / 2.0
            }
        }
        Control::Filter(d) => (st.strips[d.index()].filter + 1.0) / 2.0,
        Control::FxWet(d) => st.fx[d.index()].wet,
        Control::FxParam(d, index) => st.fx[d.index()].params.get(index).copied().unwrap_or(0.0),
    }
    .clamp(0.0, 1.0)
}

/// Actions that move more than one control at once. Everything else goes through `apply`.
pub fn apply_many(
    st: &mut ControlState,
    c: &Controls,
    snap: &Snapshot,
    action: Action,
) -> Vec<Command> {
    match action {
        Action::EqSwap(d, b) => {
            let b = band(b);
            let (mine, theirs) = (d.index(), 1 - d.index());
            st.strips[mine].kills[b as usize] = false;
            st.strips[theirs].kills[b as usize] = true;
            vec![
                Command::SetEqKill(d, b, false),
                Command::SetEqKill(d.other(), b, true),
            ]
        }
        other => apply(st, c, snap, other).into_iter().collect(),
    }
}

pub fn apply(
    st: &mut ControlState,
    c: &Controls,
    snap: &Snapshot,
    action: Action,
) -> Option<Command> {
    use Action::*;
    Some(match action {
        PlayPause(d) => Command::PlayPause(d),
        CuePress(d) => Command::CuePress(d),
        CueRelease(d) => Command::CueRelease(d),
        HotCue(d, n) => Command::HotCue(d, n),
        ClearHotCue(d, n) => Command::ClearHotCue(d, n),
        Tempo(d, dir, fine) => {
            let step = if fine {
                c.tempo_fine_step
            } else {
                c.tempo_step
            };
            let r = &mut st.rates[d.index()];
            let next =
                (*r + step * sign(dir) as f64).clamp(1.0 - c.tempo_range, 1.0 + c.tempo_range);
            // Round away float drift so repeated steps land on exact values.
            *r = (next * 1e6).round() / 1e6;
            Command::SetRate(d, *r)
        }
        SeekTenth(d, n) => {
            let frames = snap.decks[d.index()].track_frames;
            if frames == 0 {
                return None;
            }
            Command::Seek(d, frames as f64 * n as f64 / 10.0)
        }
        Fader(d, dir) => {
            let v = &mut st.faders[d.index()];
            *v = step_fader_db(*v, dir);
            Command::SetChannelFader(d, *v)
        }
        FaderEnd(d, dir) => {
            let v = &mut st.faders[d.index()];
            *v = match dir {
                Dir::Up => 1.0,
                Dir::Down => 0.0,
            };
            Command::SetChannelFader(d, *v)
        }
        CrossfaderCentre => {
            st.crossfader = 0.0;
            Command::SetCrossfader(0.0)
        }
        FilterCentre(d) => {
            st.strips[d.index()].filter = 0.0;
            Command::SetFilter(d, 0.0)
        }
        CueMix(dir) => {
            st.cue_mix = step(st.cue_mix, CUE_MIX_STEP * sign(dir), 0.0, 1.0);
            Command::SetCueMix(st.cue_mix)
        }
        HeadphoneCue(d) => {
            let on = &mut st.headphone_cue[d.index()];
            *on = !*on;
            Command::SetHeadphoneCue(d, *on)
        }
        Crossfader(dir, snap_to_end) => {
            let x = if snap_to_end {
                sign(dir)
            } else {
                st.crossfader + c.crossfader_step * sign(dir)
            };
            st.crossfader = x.clamp(-1.0, 1.0);
            Command::SetCrossfader(st.crossfader)
        }
        Trim(d, dir) => {
            let t = &mut st.strips[d.index()].trim_db;
            *t = step(*t, TRIM_STEP_DB * sign(dir), -TRIM_RANGE_DB, TRIM_RANGE_DB);
            Command::SetTrim(d, *t)
        }
        Eq(d, b, dir) => {
            let b = band(b);
            let g = &mut st.strips[d.index()].eq_db[b as usize];
            *g = step(*g, EQ_STEP_DB * sign(dir), EQ_FLOOR_DB, EQ_MAX_DB);
            Command::SetEq(d, b, *g)
        }
        EqKill(d, b) => {
            let b = band(b);
            let k = &mut st.strips[d.index()].kills[b as usize];
            *k = !*k;
            Command::SetEqKill(d, b, *k)
        }
        Filter(d, dir) => {
            let f = &mut st.strips[d.index()].filter;
            *f = step(*f, FILTER_STEP * sign(dir), -1.0, 1.0);
            // Rounding leaves -0.0 when stepping back from below; keep centre exact.
            if *f == 0.0 {
                *f = 0.0;
            }
            Command::SetFilter(d, *f)
        }
        LoopToggle(d) => {
            let i = d.index();
            if snap.decks[i].loop_span.is_some() {
                Command::SetLoop(d, None)
            } else {
                let beat = st.beat_frames[i]?;
                let start = from_here(st, snap, i);
                Command::SetLoop(d, Some((start, start + st.loop_beats[i] * beat)))
            }
        }
        LoopIn(d) => {
            let i = d.index();
            st.loop_in[i] = Some(from_here(st, snap, i));
            // Marking a new in point leaves any running loop behind.
            snap.decks[i].loop_span?;
            Command::SetLoop(d, None)
        }
        LoopOut(d) => {
            let i = d.index();
            let start = st.loop_in[i]?;
            let end = from_here(st, snap, i);
            if end <= start {
                return None;
            }
            st.loop_in[i] = None;
            // Halving and doubling carry on from the length just marked out.
            if let Some(beat) = st.beat_frames[i] {
                st.loop_beats[i] = ((end - start) / beat).clamp(MIN_LOOP_BEATS, MAX_LOOP_BEATS);
            }
            Command::SetLoop(d, Some((start, end)))
        }
        LoopHalve(d) => return scale_loop(st, snap, d, 0.5),
        LoopDouble(d) => return scale_loop(st, snap, d, 2.0),
        BeatJump(d, dir) => {
            let i = d.index();
            let beat = st.beat_frames[i]?;
            let by = st.loop_beats[i] * beat * sign(dir) as f64;
            let frames = snap.decks[i].track_frames as f64;
            Command::Seek(d, (from_here(st, snap, i) + by).clamp(0.0, frames))
        }
        FxToggle(d) => {
            let fx = &mut st.fx[d.index()];
            fx.on = !fx.on;
            Command::SetFxOn(d, fx.on)
        }
        FxNext(d) => {
            let fx = &mut st.fx[d.index()];
            fx.kind = fx.kind.next();
            Command::SetFxKind(d, fx.kind)
        }
        FxParam(d, index, dir) => {
            let fx = &mut st.fx[d.index()];
            let knob = fx.params.get_mut(index)?;
            *knob = step(*knob, FX_PARAM_STEP * sign(dir), 0.0, 1.0);
            Command::SetFxParam(d, index, *knob)
        }
        FxWet(d, dir) => {
            let fx = &mut st.fx[d.index()];
            fx.wet = step(fx.wet, FX_WET_STEP * sign(dir), 0.0, 1.0);
            Command::SetFxWet(d, fx.wet)
        }
        Nudge(d, dir) => {
            let i = d.index();
            let frames = snap.decks[i].track_frames as f64;
            let to = snap.decks[i].position + c.nudge_frames * sign(dir) as f64;
            Command::Seek(d, to.clamp(0.0, frames))
        }
        Sync(d) => {
            let (i, other) = (d.index(), 1 - d.index());
            let (mine, theirs) = (st.beat_frames[i]?, st.beat_frames[other]?);
            // Match the beat length the other deck is playing at, not the one it was cut at.
            let played = theirs / st.rates[other];
            let want = (mine / played).clamp(1.0 - c.tempo_range, 1.0 + c.tempo_range);
            if (st.rates[i] - want).abs() > 1e-9 {
                st.rates[i] = want;
                return Some(Command::SetRate(d, want));
            }
            // Tempo already matches, so this press lines the beats up.
            let phase = |deck: usize, beat: f64| {
                ((snap.decks[deck].position - st.first_beat_frames[deck]) / beat).rem_euclid(1.0)
            };
            let mut shift = phase(other, theirs) - phase(i, mine);
            // Take whichever way round is nearer, so the playhead never jumps a whole beat.
            if shift > 0.5 {
                shift -= 1.0;
            } else if shift < -0.5 {
                shift += 1.0;
            }
            let frames = snap.decks[i].track_frames as f64;
            Command::Seek(
                d,
                (snap.decks[i].position + shift * mine).clamp(0.0, frames),
            )
        }
        KeyLock(d) => {
            let on = &mut st.key_lock[d.index()];
            *on = !*on;
            Command::SetKeyLock(d, *on)
        }
        Quantize(d) => {
            let q = &mut st.quantize[d.index()];
            *q = !*q;
            return None;
        }
        _ => return None,
    })
}
