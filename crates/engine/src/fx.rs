//! One effect slot per deck, sitting after the filter and before the fader.
//!
//! Every unit runs from the same slot, so switching between them costs nothing and no
//! allocation happens after `FxSlot::new`. Units that ring, such as the echo, keep sounding
//! after the slot is switched off: the input is gated, what is already inside plays out.

use crate::dsp::{smoothing_coeff, SMOOTH_SECS};

/// Echo time as a fraction of a beat, with both knobs centred.
pub const ECHO_BEATS: f64 = 0.5;
/// How much of each repeat feeds the next one, with both knobs centred.
pub const ECHO_FEEDBACK: f64 = 0.5;
/// Shortest and longest echo the time knob reaches, in beats, as the spec asks for.
const ECHO_MIN_BEATS: f64 = 0.125;
const ECHO_MAX_BEATS: f64 = 2.0;
/// Range of the feedback knob. Short of one, so the repeats always die away.
const ECHO_MIN_FEEDBACK: f32 = 0.1;
const ECHO_MAX_FEEDBACK: f32 = 0.9;
/// How many knobs each unit has. Both stay where they are when the unit changes.
pub const FX_PARAMS: usize = 2;
/// Longest echo the delay line holds, which covers two beats down to 30 BPM.
const MAX_ECHO_SECS: f32 = 4.0;
/// Below this the tail counts as silence and the slot stops reporting that it rings.
const RING_FLOOR: f32 = 1e-4;
/// Length of one flanger sweep, in beats, with the knob centred.
pub const FLANGER_BEATS: f64 = 4.0;
/// Sweep lengths the knob reaches, in beats. Centred it lands on `FLANGER_BEATS`.
const FLANGER_MIN_BEATS: f64 = 1.0;
const FLANGER_MAX_BEATS: f64 = 16.0;
/// Shortest and longest flanger delay, in milliseconds.
const FLANGER_MIN_MS: f32 = 0.5;
const FLANGER_MAX_MS: f32 = 6.0;
/// How much of the flanged signal feeds back, which sharpens the comb.
const FLANGER_FEEDBACK: f32 = 0.35;
/// Freeverb comb delays in frames at 44.1 kHz, scaled to the session rate.
const REVERB_COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
/// Freeverb all-pass delays, same reference rate.
const REVERB_ALLPASS: [usize; 4] = [556, 441, 341, 225];
/// Frames the right channel is offset by, which is what spreads the room.
const REVERB_SPREAD: usize = 23;
const REVERB_ALLPASS_FEEDBACK: f32 = 0.5;
/// Level the whole reverb is scaled to, so a full wet mix sits beside the dry.
const REVERB_GAIN: f32 = 0.6;

/// The units the slot can run. `next` cycles in this order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FxKind {
    /// Tape-style feedback delay, timed in beats.
    #[default]
    Echo,
    /// Sweeping comb filter, its sweep timed in beats.
    Flanger,
    /// Freeverb-style room with size and damping.
    Reverb,
    /// Sample rate and bit depth reduction.
    Bitcrusher,
}

impl FxKind {
    pub fn next(self) -> Self {
        match self {
            Self::Echo => Self::Flanger,
            Self::Flanger => Self::Reverb,
            Self::Reverb => Self::Bitcrusher,
            Self::Bitcrusher => Self::Echo,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Echo => "Echo",
            Self::Flanger => "Flanger",
            Self::Reverb => "Reverb",
            Self::Bitcrusher => "Bitcrusher",
        }
    }
}

/// Map a knob at 0..1 onto a range, geometrically so the centre lands on the geometric mean
/// and each half of the travel feels the same. Used for times, which the ear hears in ratios.
fn geometric(knob: f32, min: f64, max: f64) -> f64 {
    min * (max / min).powf(knob.clamp(0.0, 1.0) as f64)
}

fn linear(knob: f32, min: f32, max: f32) -> f32 {
    min + (max - min) * knob.clamp(0.0, 1.0)
}

/// One-pole ramp for gains that would otherwise click. Snaps once it is close enough,
/// so a settled gain is exactly its target and multiplying by it changes nothing.
#[derive(Clone, Copy)]
struct Ramp {
    value: f32,
    target: f32,
    smooth: f32,
}

impl Ramp {
    fn new(smooth: f32) -> Self {
        Self {
            value: 0.0,
            target: 0.0,
            smooth,
        }
    }

    fn set(&mut self, target: f32) {
        self.target = target;
    }

    fn next(&mut self) -> f32 {
        self.value += (self.target - self.value) * self.smooth;
        // A gain this close to its target is 80 dB from wrong, and snapping makes a bypass
        // exact instead of forever approaching it.
        if (self.target - self.value).abs() < 1e-4 {
            self.value = self.target;
        }
        self.value
    }

    fn is(&self, v: f32) -> bool {
        self.value == v && self.target == v
    }
}

/// Feedback delay with the repeats timed from the beat length the slot is given.
struct Echo {
    line: Vec<f32>,
    write: usize,
    /// Frames left before the line is certainly silent. Sound written into the line sets
    /// this to twice the delay, which covers the gap before it is read back.
    hot: usize,
}

impl Echo {
    fn new(fs: f32) -> Self {
        let frames = (fs * MAX_ECHO_SECS) as usize;
        Self {
            line: vec![0.0; frames * 2],
            write: 0,
            hot: 0,
        }
    }

    /// Add the repeats to `buf`. `feed` gates what enters the line, 1 while the slot is on.
    fn process(&mut self, buf: &mut [f32], delay_frames: usize, feedback: f32, feed: &mut Ramp) {
        let frames = self.line.len() / 2;
        let delay = delay_frames.clamp(1, frames - 1);
        for frame in buf.as_chunks_mut::<2>().0 {
            let read = (self.write + frames - delay) % frames;
            let (l, r) = (self.line[read * 2], self.line[read * 2 + 1]);
            let feed = feed.next();
            let (wl, wr) = (
                frame[0] * feed + l * feedback,
                frame[1] * feed + r * feedback,
            );
            self.line[self.write * 2] = wl;
            self.line[self.write * 2 + 1] = wr;
            frame[0] += l;
            frame[1] += r;
            self.hot = if wl.abs().max(wr.abs()) > RING_FLOOR {
                delay * 2
            } else {
                self.hot.saturating_sub(1)
            };
            self.write = (self.write + 1) % frames;
        }
    }

    fn rings(&self) -> bool {
        self.hot > 0
    }

    fn clear(&mut self) {
        self.line.fill(0.0);
        self.hot = 0;
    }
}

/// Comb filter whose delay sweeps between two short times, once per `FLANGER_BEATS`.
struct Flanger {
    line: Vec<f32>,
    write: usize,
    /// Sweep position, 0 to 1 over one period.
    phase: f32,
    min_frames: f32,
    max_frames: f32,
}

impl Flanger {
    fn new(fs: f32) -> Self {
        let max = fs * FLANGER_MAX_MS / 1000.0;
        Self {
            line: vec![0.0; (max.ceil() as usize + 2) * 2],
            write: 0,
            phase: 0.0,
            min_frames: fs * FLANGER_MIN_MS / 1000.0,
            max_frames: max,
        }
    }

    /// Linear read between the two samples either side of a fractional delay.
    fn read(&self, delay: f32, channel: usize) -> f32 {
        let frames = self.line.len() / 2;
        let back = delay.clamp(1.0, frames as f32 - 2.0);
        let whole = back.floor();
        let frac = back - whole;
        let i = (self.write + frames - whole as usize) % frames;
        let j = (i + frames - 1) % frames;
        self.line[i * 2 + channel] * (1.0 - frac) + self.line[j * 2 + channel] * frac
    }

    fn process(&mut self, buf: &mut [f32], period_frames: f32, depth: f32, gate: &mut Ramp) {
        let frames = self.line.len() / 2;
        let step = 1.0 / period_frames.max(1.0);
        let span = (self.max_frames - self.min_frames) * depth.clamp(0.0, 1.0);
        for frame in buf.as_chunks_mut::<2>().0 {
            // Triangle sweep: down and back up, so the turn at each end is not a jump.
            let tri = 1.0 - (self.phase * 2.0 - 1.0).abs();
            let delay = self.min_frames + span * tri;
            let gate = gate.next();
            for (ch, sample) in frame.iter_mut().enumerate() {
                let delayed = self.read(delay, ch);
                self.line[self.write * 2 + ch] = *sample * gate + delayed * FLANGER_FEEDBACK;
                *sample += delayed * gate;
            }
            self.write = (self.write + 1) % frames;
            self.phase = (self.phase + step).fract();
        }
    }

    fn clear(&mut self) {
        self.line.fill(0.0);
        self.phase = 0.0;
        self.write = 0;
    }
}

/// One damped comb: a delay whose feedback runs through a one-pole low-pass, which is
/// what makes a Freeverb tail lose its highs as it decays.
struct Comb {
    line: Vec<f32>,
    index: usize,
    store: f32,
}

impl Comb {
    fn new(frames: usize) -> Self {
        Self {
            line: vec![0.0; frames.max(1)],
            index: 0,
            store: 0.0,
        }
    }

    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let out = self.line[self.index];
        self.store = out * (1.0 - damp) + self.store * damp;
        self.line[self.index] = input + self.store * feedback;
        self.index = (self.index + 1) % self.line.len();
        out
    }

    fn clear(&mut self) {
        self.line.fill(0.0);
        self.store = 0.0;
    }
}

/// Schroeder all-pass, which smears the comb output without colouring it.
struct Allpass {
    line: Vec<f32>,
    index: usize,
}

impl Allpass {
    fn new(frames: usize) -> Self {
        Self {
            line: vec![0.0; frames.max(1)],
            index: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buffered = self.line[self.index];
        let out = buffered - input;
        self.line[self.index] = input + buffered * REVERB_ALLPASS_FEEDBACK;
        self.index = (self.index + 1) % self.line.len();
        out
    }

    fn clear(&mut self) {
        self.line.fill(0.0);
    }
}

/// Freeverb: eight damped combs in parallel into four all-passes in series, per channel.
struct Reverb {
    combs: [[Comb; 8]; 2],
    allpass: [[Allpass; 4]; 2],
    size: f32,
    damping: f32,
    /// Peak of the last block's wet output, which is how the slot knows it still rings.
    ring: f32,
}

impl Reverb {
    fn new(fs: f32) -> Self {
        let scale =
            |frames: usize, offset: usize| ((frames + offset) as f32 * fs / 44_100.0) as usize;
        Self {
            combs: [
                std::array::from_fn(|i| Comb::new(scale(REVERB_COMBS[i], 0))),
                std::array::from_fn(|i| Comb::new(scale(REVERB_COMBS[i], REVERB_SPREAD))),
            ],
            allpass: [
                std::array::from_fn(|i| Allpass::new(scale(REVERB_ALLPASS[i], 0))),
                std::array::from_fn(|i| Allpass::new(scale(REVERB_ALLPASS[i], REVERB_SPREAD))),
            ],
            size: 0.7,
            damping: 0.4,
            ring: 0.0,
        }
    }

    fn set_size(&mut self, size: f32) {
        self.size = size.clamp(0.0, 1.0);
    }

    fn set_damping(&mut self, damping: f32) {
        self.damping = damping.clamp(0.0, 1.0);
    }

    fn process(&mut self, buf: &mut [f32], gate: &mut Ramp) {
        let feedback = 0.7 + self.size * 0.28;
        let damp = self.damping * 0.4;
        let mut ring = 0.0f32;
        for frame in buf.as_chunks_mut::<2>().0 {
            let gate = gate.next();
            for ((sample, combs), allpass) in frame
                .iter_mut()
                .zip(self.combs.iter_mut())
                .zip(self.allpass.iter_mut())
            {
                let input = *sample * gate * REVERB_GAIN;
                let mut wet = 0.0;
                for comb in combs.iter_mut() {
                    wet += comb.process(input, feedback, damp);
                }
                for ap in allpass.iter_mut() {
                    wet = ap.process(wet);
                }
                ring = ring.max(wet.abs());
                *sample = wet;
            }
        }
        self.ring = ring;
    }

    fn rings(&self) -> bool {
        self.ring > RING_FLOOR
    }

    fn clear(&mut self) {
        for ch in 0..2 {
            for comb in &mut self.combs[ch] {
                comb.clear();
            }
            for ap in &mut self.allpass[ch] {
                ap.clear();
            }
        }
        self.ring = 0.0;
    }
}

/// Sample rate reduction with bit depth reduction on top.
#[derive(Default)]
struct Bitcrusher {
    held: [f32; 2],
    counter: usize,
}

impl Bitcrusher {
    fn process(&mut self, buf: &mut [f32], bits: u32, hold: usize) {
        let levels = ((1u32 << bits.clamp(1, 16)) - 1) as f32;
        let hold = hold.max(1);
        for frame in buf.as_chunks_mut::<2>().0 {
            if self.counter == 0 {
                self.held = [
                    (frame[0].clamp(-1.0, 1.0) * levels).round() / levels,
                    (frame[1].clamp(-1.0, 1.0) * levels).round() / levels,
                ];
            }
            self.counter = (self.counter + 1) % hold;
            frame[0] = self.held[0];
            frame[1] = self.held[1];
        }
    }

    fn reset(&mut self) {
        self.held = [0.0; 2];
        self.counter = 0;
    }
}

/// The effect slot for one channel.
pub struct FxSlot {
    kind: FxKind,
    on: bool,
    wet: f32,
    /// Smoothed dry/wet, so turning the knob never clicks.
    mix: Ramp,
    /// Smoothed input gate, 1 while on. What is already inside plays out as this falls.
    feed: Ramp,
    beat_frames: f32,
    /// Two knobs, each 0 to 1, read differently by each unit.
    params: [f32; FX_PARAMS],
    echo: Echo,
    flanger: Flanger,
    reverb: Reverb,
    crusher: Bitcrusher,
    /// Scratch copy of the dry signal, for the crossfade back into the block.
    dry: Vec<f32>,
}

impl FxSlot {
    pub fn new(fs: f32) -> Self {
        Self {
            kind: FxKind::default(),
            on: false,
            wet: 0.0,
            mix: Ramp::new(smoothing_coeff(fs, SMOOTH_SECS)),
            feed: Ramp::new(smoothing_coeff(fs, SMOOTH_SECS)),
            beat_frames: fs / 2.0,
            params: [0.5; FX_PARAMS],
            echo: Echo::new(fs),
            flanger: Flanger::new(fs),
            reverb: Reverb::new(fs),
            crusher: Bitcrusher::default(),
            dry: vec![0.0; crate::MAX_BLOCK_FRAMES * 2],
        }
    }

    pub fn kind(&self) -> FxKind {
        self.kind
    }

    /// Switch units. The unit being left behind is silenced rather than left ringing into
    /// the next one.
    pub fn set_kind(&mut self, kind: FxKind) {
        if kind != self.kind {
            self.echo.clear();
            self.flanger.clear();
            self.reverb.clear();
            self.crusher.reset();
            self.kind = kind;
        }
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    pub fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    pub fn wet(&self) -> f32 {
        self.wet
    }

    pub fn set_wet(&mut self, wet: f32) {
        self.wet = wet.clamp(0.0, 1.0);
    }

    /// Frames in one beat at the tempo the deck is playing.
    pub fn set_beat_frames(&mut self, frames: f32) {
        if frames.is_finite() && frames > 0.0 {
            self.beat_frames = frames;
        }
    }

    /// Knob `index`, 0 to 1. What it means depends on the unit: see [`FxSlot::param_name`].
    pub fn param(&self, index: usize) -> f32 {
        self.params.get(index).copied().unwrap_or(0.0)
    }

    pub fn set_param(&mut self, index: usize, value: f32) {
        if let Some(p) = self.params.get_mut(index) {
            *p = value.clamp(0.0, 1.0);
        }
    }

    /// What knob `index` does in the unit now in the slot.
    pub fn param_name(&self, index: usize) -> &'static str {
        match (self.kind, index) {
            (FxKind::Echo, 0) => "Time",
            (FxKind::Echo, _) => "Feedback",
            (FxKind::Flanger, 0) => "Sweep",
            (FxKind::Flanger, _) => "Depth",
            (FxKind::Reverb, 0) => "Size",
            (FxKind::Reverb, _) => "Damping",
            (FxKind::Bitcrusher, 0) => "Bits",
            (FxKind::Bitcrusher, _) => "Rate",
        }
    }

    /// Room size, 0 for a small space and 1 for a long tail.
    pub fn set_reverb_size(&mut self, size: f32) {
        self.set_param(0, size);
    }

    /// How fast the tail loses its high end.
    pub fn set_reverb_damping(&mut self, damping: f32) {
        self.set_param(1, damping);
    }

    /// True while a unit still has something to play after being switched off.
    pub fn is_ringing(&self) -> bool {
        !self.on
            && match self.kind {
                FxKind::Echo => self.echo.rings(),
                FxKind::Reverb => self.reverb.rings(),
                FxKind::Flanger | FxKind::Bitcrusher => false,
            }
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        let target = if self.on { self.wet } else { 0.0 };
        self.mix.set(target);
        self.feed.set(self.on as u8 as f32);
        if self.mix.is(0.0) && self.feed.is(0.0) && !self.is_ringing() {
            return;
        }
        // The scratch copy of the dry signal bounds how much is processed at a time.
        let block = self.dry.len();
        for chunk in buf.chunks_mut(block) {
            self.process_block(chunk);
        }
    }

    fn process_block(&mut self, buf: &mut [f32]) {
        let n = buf.len();
        self.dry[..n].copy_from_slice(buf);

        // The gate reaches the units; the mix only decides how much comes back.
        let [knob, knob2] = self.params;
        match self.kind {
            FxKind::Echo => {
                let beats = geometric(knob, ECHO_MIN_BEATS, ECHO_MAX_BEATS);
                let delay = (beats * self.beat_frames as f64) as usize;
                let feedback = linear(knob2, ECHO_MIN_FEEDBACK, ECHO_MAX_FEEDBACK);
                self.echo.process(buf, delay, feedback, &mut self.feed);
            }
            FxKind::Flanger => {
                let beats = geometric(knob, FLANGER_MIN_BEATS, FLANGER_MAX_BEATS);
                let period = (beats * self.beat_frames as f64) as f32;
                self.flanger.process(buf, period, knob2, &mut self.feed);
            }
            FxKind::Reverb => {
                self.reverb.set_size(knob);
                self.reverb.set_damping(knob2);
                self.reverb.process(buf, &mut self.feed);
            }
            FxKind::Bitcrusher => {
                let bits = linear(1.0 - knob, 1.0, 9.0).round() as u32;
                let hold = linear(knob2, 1.0, 47.0).round() as usize;
                self.crusher.process(buf, bits, hold);
            }
        }

        // A ringing echo keeps the wet level it had, so switching off does not cut the tail.
        let hold = self.is_ringing();
        for (i, frame) in buf.as_chunks_mut::<2>().0.iter_mut().enumerate() {
            let mix = if hold {
                self.mix.value
            } else {
                self.mix.next()
            };
            let dry = [self.dry[i * 2], self.dry[i * 2 + 1]];
            frame[0] = dry[0] + (frame[0] - dry[0]) * mix;
            frame[1] = dry[1] + (frame[1] - dry[1]) * mix;
        }
    }
}
