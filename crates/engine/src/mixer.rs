use crate::{Deck, Track};
use std::sync::Arc;
use std::f32::consts::FRAC_PI_2;

/// Largest block rendered in one pass. Bigger callbacks are split, so no allocation happens in `process`.
pub const MAX_BLOCK_FRAMES: usize = 4096;

/// Width of the fade region at each end of the cut curve.
const CUT_WIDTH: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeckId {
    A,
    B,
}

impl DeckId {
    pub fn other(self) -> Self {
        match self {
            DeckId::A => DeckId::B,
            DeckId::B => DeckId::A,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CrossfaderCurve {
    Linear,
    #[default]
    ConstantPower,
    /// Both channels at full level except near the ends, for scratch mixing.
    Cut,
}

/// Gains for decks A and B at crossfader position `x`, where -1 is fully A and 1 is fully B.
pub fn crossfader_gains(x: f32, curve: CrossfaderCurve) -> (f32, f32) {
    let x = x.clamp(-1.0, 1.0);
    match curve {
        CrossfaderCurve::Linear => ((1.0 - x) * 0.5, (1.0 + x) * 0.5),
        CrossfaderCurve::ConstantPower => {
            let t = (x + 1.0) * 0.5 * FRAC_PI_2;
            (t.cos().max(0.0), t.sin().max(0.0))
        }
        CrossfaderCurve::Cut => (
            ((1.0 - x) / CUT_WIDTH).min(1.0),
            ((1.0 + x) / CUT_WIDTH).min(1.0),
        ),
    }
}

#[derive(Clone, Copy)]
struct Channel {
    fader: f32,
    headphone_cue: bool,
}

impl Default for Channel {
    fn default() -> Self {
        Self { fader: 1.0, headphone_cue: false }
    }
}

/// Two decks feeding a DJ mixer with a master bus and a headphone cue bus.
pub struct Engine {
    decks: [Deck; 2],
    channels: [Channel; 2],
    crossfader: f32,
    curve: CrossfaderCurve,
    /// 0 is cue bus only, 1 is master only.
    cue_mix: f32,
    scratch: [Vec<f32>; 2],
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            decks: [Deck::new(), Deck::new()],
            channels: [Channel::default(); 2],
            crossfader: 0.0,
            curve: CrossfaderCurve::default(),
            cue_mix: 0.5,
            scratch: [vec![0.0; MAX_BLOCK_FRAMES * 2], vec![0.0; MAX_BLOCK_FRAMES * 2]],
        }
    }

    pub fn deck(&self, id: DeckId) -> &Deck {
        &self.decks[id.index()]
    }

    pub fn deck_mut(&mut self, id: DeckId) -> &mut Deck {
        &mut self.decks[id.index()]
    }

    pub fn crossfader(&self) -> f32 {
        self.crossfader
    }

    pub fn set_crossfader(&mut self, x: f32) {
        self.crossfader = x.clamp(-1.0, 1.0);
    }

    pub fn set_crossfader_curve(&mut self, curve: CrossfaderCurve) {
        self.curve = curve;
    }

    pub fn channel_fader(&self, id: DeckId) -> f32 {
        self.channels[id.index()].fader
    }

    pub fn set_channel_fader(&mut self, id: DeckId, v: f32) {
        self.channels[id.index()].fader = v.clamp(0.0, 1.0);
    }

    pub fn headphone_cue(&self, id: DeckId) -> bool {
        self.channels[id.index()].headphone_cue
    }

    pub fn set_headphone_cue(&mut self, id: DeckId, on: bool) {
        self.channels[id.index()].headphone_cue = on;
    }

    pub fn set_cue_mix(&mut self, v: f32) {
        self.cue_mix = v.clamp(0.0, 1.0);
    }

    pub fn crossfader_curve(&self) -> CrossfaderCurve {
        self.curve
    }

    pub fn cue_mix(&self) -> f32 {
        self.cue_mix
    }

    /// Apply a command. A replaced track is returned so it can be freed off the audio thread.
    pub fn apply(&mut self, cmd: crate::Command) -> Option<Arc<Track>> {
        use crate::Command::*;
        match cmd {
            Load(d, t) => return self.deck_mut(d).load(t),
            PlayPause(d) => self.deck_mut(d).play_pause(),
            CuePress(d) => self.deck_mut(d).cue_press(),
            CueRelease(d) => self.deck_mut(d).cue_release(),
            HotCue(d, n) => self.deck_mut(d).hot_cue(n),
            ClearHotCue(d, n) => self.deck_mut(d).clear_hot_cue(n),
            Seek(d, f) => self.deck_mut(d).seek(f),
            SetRate(d, r) => self.deck_mut(d).set_rate(r),
            SetChannelFader(d, v) => self.set_channel_fader(d, v),
            SetCrossfader(x) => self.set_crossfader(x),
            SetCrossfaderCurve(c) => self.set_crossfader_curve(c),
            SetHeadphoneCue(d, on) => self.set_headphone_cue(d, on),
            SetCueMix(v) => self.set_cue_mix(v),
        }
        None
    }

    /// Fill interleaved stereo `master` and `cue` buffers of equal length.
    pub fn process(&mut self, master: &mut [f32], cue: &mut [f32]) {
        assert_eq!(master.len(), cue.len(), "master and cue buffers must match");
        let (xa, xb) = crossfader_gains(self.crossfader, self.curve);
        let xf = [xa, xb];
        for (m_block, c_block) in master
            .chunks_mut(MAX_BLOCK_FRAMES * 2)
            .zip(cue.chunks_mut(MAX_BLOCK_FRAMES * 2))
        {
            let n = m_block.len();
            for (deck, buf) in self.decks.iter_mut().zip(self.scratch.iter_mut()) {
                deck.render(&mut buf[..n]);
            }
            let post = [self.channels[0].fader * xf[0], self.channels[1].fader * xf[1]];
            let pfl = [
                self.channels[0].headphone_cue as u8 as f32,
                self.channels[1].headphone_cue as u8 as f32,
            ];
            let (a, b) = (&self.scratch[0][..n], &self.scratch[1][..n]);
            for i in 0..n {
                let m = a[i] * post[0] + b[i] * post[1];
                let c = a[i] * pfl[0] + b[i] * pfl[1];
                m_block[i] = m;
                c_block[i] = c * (1.0 - self.cue_mix) + m * self.cue_mix;
            }
        }
    }
}
