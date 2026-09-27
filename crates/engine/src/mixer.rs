use crate::dsp::{DjFilter, Isolator, Trim};
use crate::{Deck, Track};
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

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

/// One mixer channel: trim, isolator EQ and filter ahead of the fader.
struct Channel {
    fader: f32,
    headphone_cue: bool,
    trim: Trim,
    eq: Isolator,
    filter: DjFilter,
    fx: crate::fx::FxSlot,
    /// Beat length of the track as analysed, before the tempo fader.
    beat_frames: f32,
}

impl Channel {
    fn new(fs: f32) -> Self {
        Self {
            fader: 1.0,
            headphone_cue: false,
            trim: Trim::new(fs),
            eq: Isolator::new(fs),
            filter: DjFilter::new(fs),
            fx: crate::fx::FxSlot::new(fs),
            beat_frames: fs / 2.0,
        }
    }
}

/// Peak levels since the last call to `Engine::take_peaks`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Meters {
    /// Per channel, after EQ and filter, before the fader.
    pub channels: [f32; 2],
    /// Master bus.
    pub master: f32,
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
    peaks: Meters,
    limiter: crate::dsp::Limiter,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// An engine at 48 kHz. Use [`Engine::with_sample_rate`] for anything else.
    pub fn new() -> Self {
        Self::with_sample_rate(48_000)
    }

    pub fn with_sample_rate(sample_rate: u32) -> Self {
        let fs = sample_rate as f32;
        Self {
            decks: [
                Deck::with_sample_rate(sample_rate),
                Deck::with_sample_rate(sample_rate),
            ],
            channels: [Channel::new(fs), Channel::new(fs)],
            peaks: Meters::default(),
            crossfader: 0.0,
            curve: CrossfaderCurve::default(),
            cue_mix: 0.5,
            scratch: [
                vec![0.0; MAX_BLOCK_FRAMES * 2],
                vec![0.0; MAX_BLOCK_FRAMES * 2],
            ],
            limiter: crate::dsp::Limiter::new(fs),
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

    /// Beat length the effect slot is working to, after the tempo fader.
    pub fn fx_beat_frames(&self, id: DeckId) -> f32 {
        let ch = &self.channels[id.index()];
        ch.beat_frames / self.decks[id.index()].rate().max(1e-3) as f32
    }

    pub fn fx(&self, id: DeckId) -> &crate::fx::FxSlot {
        &self.channels[id.index()].fx
    }

    pub fn crossfader_curve(&self) -> CrossfaderCurve {
        self.curve
    }

    pub fn cue_mix(&self) -> f32 {
        self.cue_mix
    }

    /// How far the master limiter pulled the loudest moment down since the last call, in dB.
    pub fn limiter_reduction_db(&mut self) -> f32 {
        self.limiter.reduction_db()
    }

    /// Peaks since the previous call, then reset.
    pub fn take_peaks(&mut self) -> Meters {
        std::mem::take(&mut self.peaks)
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
            SetTrim(d, db) => self.channels[d.index()].trim.set_db(db),
            SetEq(d, band, db) => self.channels[d.index()].eq.set_gain_db(band, db),
            SetEqKill(d, band, kill) => self.channels[d.index()].eq.set_kill(band, kill),
            SetFilter(d, v) => self.channels[d.index()].filter.set(v),
            SetCuePoint(d, f) => self.deck_mut(d).set_cue_point(f),
            SetHotCue(d, n, f) => self.deck_mut(d).set_hot_cue(n, f),
            SetLoop(d, span) => self.deck_mut(d).set_loop(span),
            SetFxKind(d, kind) => self.channels[d.index()].fx.set_kind(kind),
            SetFxOn(d, on) => self.channels[d.index()].fx.set_on(on),
            SetFxWet(d, wet) => self.channels[d.index()].fx.set_wet(wet),
            SetFxParam(d, i, v) => self.channels[d.index()].fx.set_param(i, v),
            SetBeatFrames(d, frames) => self.channels[d.index()].beat_frames = frames,
            SetKeyLock(d, on) => self.deck_mut(d).set_key_lock(on),
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
            for ((deck, ch), (buf, peak)) in self
                .decks
                .iter_mut()
                .zip(self.channels.iter_mut())
                .zip(self.scratch.iter_mut().zip(self.peaks.channels.iter_mut()))
            {
                let buf = &mut buf[..n];
                deck.render(buf);
                ch.trim.process(buf);
                ch.eq.process(buf);
                ch.filter.process(buf);
                // A track played faster has shorter beats, so the echo shortens with it.
                let rate = deck.rate().max(1e-3) as f32;
                ch.fx.set_beat_frames(ch.beat_frames / rate);
                ch.fx.process(buf);
                *peak = buf.iter().fold(*peak, |m, s| m.max(s.abs()));
            }
            let post = [
                self.channels[0].fader * xf[0],
                self.channels[1].fader * xf[1],
            ];
            let pfl = [
                self.channels[0].headphone_cue as u8 as f32,
                self.channels[1].headphone_cue as u8 as f32,
            ];
            let (a, b) = (&self.scratch[0][..n], &self.scratch[1][..n]);
            for i in 0..n {
                m_block[i] = a[i] * post[0] + b[i] * post[1];
                c_block[i] = a[i] * pfl[0] + b[i] * pfl[1];
            }
            // The limiter is the last thing on the master bus, and the cue bus blends in
            // what the master actually puts out rather than the sum before it.
            self.limiter.process(m_block);
            for i in 0..n {
                self.peaks.master = self.peaks.master.max(m_block[i].abs());
                c_block[i] = c_block[i] * (1.0 - self.cue_mix) + m_block[i] * self.cue_mix;
            }
        }
    }
}
