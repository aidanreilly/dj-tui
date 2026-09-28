//! Fades that run on their own once a key starts them.
//!
//! A keyboard cannot sweep a control, so it starts a sweep and walks away. Everything here
//! runs on the UI thread and is driven by the frame delta, which is why it needs no clock and
//! is tested with numbers. Values go out through `apply::set_control`, so `ControlState` stays
//! the only owner of absolute values and MIDI soft takeover keeps working.

use engine::DeckId;
use input::{Control, Dir};

/// Shortest and longest fade the keys can ask for, in beats.
pub const MIN_FADE_BEATS: f64 = 2.0;
pub const MAX_FADE_BEATS: f64 = 64.0;
/// Floor on the wall-clock length. The mixer applies fader gains as plain scalars with no
/// per-sample ramp, so a fade is a series of steps; at 30 frames a second this keeps each one
/// no larger than the manual key step.
pub const MIN_FADE_SECS: f64 = 0.7;
/// A beat on a deck with no grid, which is 120 BPM.
pub const NO_GRID_BEAT_SECS: f64 = 0.5;
/// How far down a decibel fade reaches before it goes to silence.
const FADE_FLOOR_DB: f32 = -60.0;

/// How a fade interpolates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    /// Straight down the control's own travel. For the crossfader, whose configured curve is
    /// already downstream, and for the filter.
    Position,
    /// Even in decibels. For channel faders, which take raw amplitude, where a linear ramp
    /// loses 6 dB in the first half of the travel and the rest in a rush at the end.
    Decibel,
}

#[derive(Debug, Clone, Copy)]
struct Fade {
    control: Control,
    from: f32,
    to: f32,
    elapsed: f64,
    secs: f64,
    curve: Curve,
}

impl Fade {
    /// Where this fade sits, and whether it has finished.
    fn value(&self) -> (f32, bool) {
        let t = (self.elapsed / self.secs).clamp(0.0, 1.0);
        if t >= 1.0 {
            return (self.to, true);
        }
        let v = match self.curve {
            Curve::Position => self.from + (self.to - self.from) * t as f32,
            Curve::Decibel => {
                let db = |v: f32| {
                    if v <= 0.0 {
                        FADE_FLOOR_DB
                    } else {
                        (20.0 * v.log10()).max(FADE_FLOOR_DB)
                    }
                };
                let at = db(self.from) + (db(self.to) - db(self.from)) * t as f32;
                10f32.powf(at / 20.0)
            }
        };
        (v, false)
    }
}

pub struct Automation {
    fades: Vec<Fade>,
    fade_beats: f64,
}

impl Automation {
    pub fn new(fade_beats: f64) -> Self {
        Self {
            fades: Vec::with_capacity(8),
            fade_beats: fade_beats.clamp(MIN_FADE_BEATS, MAX_FADE_BEATS),
        }
    }

    pub fn fade_beats(&self) -> f64 {
        self.fade_beats
    }

    /// Halve or double the shared fade length, within the range.
    pub fn scale_length(&mut self, dir: Dir) {
        let by = match dir {
            Dir::Up => 2.0,
            Dir::Down => 0.5,
        };
        self.fade_beats = (self.fade_beats * by).clamp(MIN_FADE_BEATS, MAX_FADE_BEATS);
    }

    /// How long a fade lasts on a deck with this beat length and rate. Worked out when the
    /// fade starts, so moving the tempo fader part way through does not stretch it.
    pub fn secs_for(&self, beat_frames: Option<f64>, rate: f64, sample_rate: u32) -> f64 {
        let beat_secs = match beat_frames {
            Some(frames) => frames / sample_rate as f64 / rate.max(1e-3),
            None => NO_GRID_BEAT_SECS,
        };
        (self.fade_beats * beat_secs).max(MIN_FADE_SECS)
    }

    /// Begin a fade, replacing any other on the same control. A fade that would not move
    /// anything is not started.
    pub fn start(&mut self, control: Control, from: f32, to: f32, secs: f64, curve: Curve) {
        self.cancel(control);
        if (from - to).abs() < 1e-6 || secs <= 0.0 {
            return;
        }
        self.fades.push(Fade {
            control,
            from,
            to,
            elapsed: 0.0,
            secs,
            curve,
        });
    }

    /// Advance every fade by `dt` seconds, pushing the value each one reached and dropping the
    /// ones that finished.
    pub fn tick(&mut self, dt: f64, out: &mut Vec<(Control, f32)>) {
        self.fades.retain_mut(|f| {
            f.elapsed += dt;
            let (v, done) = f.value();
            out.push((f.control, v));
            !done
        });
    }

    pub fn cancel(&mut self, control: Control) {
        self.fades.retain(|f| f.control != control);
    }

    /// Stop every fade on one deck's controls, for a track being replaced under them.
    pub fn cancel_deck(&mut self, deck: DeckId) {
        self.fades.retain(|f| match f.control {
            Control::Fader(d)
            | Control::Filter(d)
            | Control::Trim(d)
            | Control::Tempo(d)
            | Control::Eq(d, _)
            | Control::FxWet(d)
            | Control::FxParam(d, _) => d != deck,
            Control::Crossfader | Control::CueMix => true,
        });
    }

    pub fn cancel_all(&mut self) {
        self.fades.clear();
    }

    /// Where a running fade is heading, for the marker on the fader.
    pub fn target(&self, control: Control) -> Option<f32> {
        self.fades
            .iter()
            .find(|f| f.control == control)
            .map(|f| f.to)
    }

    /// How many fades are live. `Esc` reports nothing when this is zero.
    pub fn running(&self) -> usize {
        self.fades.len()
    }
}
