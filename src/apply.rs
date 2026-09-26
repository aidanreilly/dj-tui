//! Maps input actions onto engine calls. Actions for features not built yet return `false`.

use engine::Engine;
use input::{Action, Dir};

/// Step sizes for keyboard controls.
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

fn sign(d: Dir) -> f32 {
    match d {
        Dir::Up => 1.0,
        Dir::Down => -1.0,
    }
}

/// Apply one action. Returns whether it did anything.
pub fn apply(e: &mut Engine, c: &Controls, action: Action) -> bool {
    use Action::*;
    match action {
        PlayPause(d) => e.deck_mut(d).play_pause(),
        CuePress(d) => e.deck_mut(d).cue_press(),
        CueRelease(d) => e.deck_mut(d).cue_release(),
        HotCue(d, n) => e.deck_mut(d).hot_cue(n),
        ClearHotCue(d, n) => e.deck_mut(d).clear_hot_cue(n),
        Tempo(d, dir, fine) => {
            let step = if fine { c.tempo_fine_step } else { c.tempo_step };
            let deck = e.deck_mut(d);
            let rate = (deck.rate() + step * sign(dir) as f64)
                .clamp(1.0 - c.tempo_range, 1.0 + c.tempo_range);
            // Round away float drift so repeated steps land on exact values.
            deck.set_rate((rate * 1e6).round() / 1e6);
        }
        SeekTenth(d, n) => {
            let deck = e.deck_mut(d);
            let frames = deck.track().map_or(0, |t| t.frames()) as f64;
            deck.seek(frames * n as f64 / 10.0);
        }
        Fader(d, dir) => {
            let v = e.channel_fader(d) + c.fader_step * sign(dir);
            e.set_channel_fader(d, v);
        }
        HeadphoneCue(d) => {
            let on = !e.headphone_cue(d);
            e.set_headphone_cue(d, on);
        }
        Crossfader(dir, snap) => {
            let x = if snap { sign(dir) } else { e.crossfader() + c.crossfader_step * sign(dir) };
            e.set_crossfader(x);
        }
        _ => return false,
    }
    true
}
