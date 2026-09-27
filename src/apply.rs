//! Maps input actions onto engine commands.
//!
//! The UI owns the absolute value of every continuous control (faders, crossfader, tempo) in
//! `ControlState`, so several key presses between two audio callbacks each count.
//! Actions for features not built yet produce no command.

use engine::dsp::{EqBand, EQ_MAX_DB, TRIM_RANGE_DB};
use engine::{Command, Snapshot};
use input::{Action, Dir};

/// Step sizes and limits for keyboard controls.
#[derive(Debug, Clone)]
pub struct Controls {
    pub tempo_range: f64,
    pub tempo_step: f64,
    pub tempo_fine_step: f64,
    pub crossfader_step: f32,
    pub fader_step: f32,
}

impl Controls {
    /// `tempo_range_percent` is the configured fader range (8, 16 or 50).
    pub fn new(tempo_range_percent: u8) -> Self {
        Self {
            tempo_range: tempo_range_percent as f64 / 100.0,
            tempo_step: 0.005,
            tempo_fine_step: 0.0005,
            crossfader_step: 0.1,
            fader_step: 0.05,
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
}

/// Trim, EQ and filter for one channel. Arrays are indexed by `EqBand as usize`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StripState {
    pub trim_db: f32,
    pub eq_db: [f32; 3],
    pub kills: [bool; 3],
    pub filter: f32,
}

pub const TRIM_STEP_DB: f32 = 1.0;
pub const EQ_STEP_DB: f32 = 2.0;
/// Lowest EQ knob position short of a kill.
pub const EQ_FLOOR_DB: f32 = -26.0;
pub const FILTER_STEP: f32 = 0.1;

fn band(b: input::Band) -> EqBand {
    match b {
        input::Band::Low => EqBand::Low,
        input::Band::Mid => EqBand::Mid,
        input::Band::High => EqBand::High,
    }
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
        }
    }
}

fn sign(d: Dir) -> f32 {
    match d {
        Dir::Up => 1.0,
        Dir::Down => -1.0,
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
            *v = (*v + c.fader_step * sign(dir)).clamp(0.0, 1.0);
            Command::SetChannelFader(d, *v)
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
        _ => return None,
    })
}
