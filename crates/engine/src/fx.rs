//! One effect slot per deck, sitting after the filter and before the fader.
//!
//! Every unit runs from the same slot, so switching between them costs nothing and no
//! allocation happens after `FxSlot::new`. Units that ring, such as the echo, keep sounding
//! after the slot is switched off: the input is gated, what is already inside plays out.

use crate::dsp::{smoothing_coeff, SMOOTH_SECS};

/// Echo time as a fraction of a beat.
pub const ECHO_BEATS: f64 = 0.5;
/// How much of each repeat feeds the next one.
pub const ECHO_FEEDBACK: f64 = 0.5;
/// Longest echo the delay line holds, which covers two beats down to 30 BPM.
const MAX_ECHO_SECS: f32 = 4.0;
/// Below this the tail counts as silence and the slot stops reporting that it rings.
const RING_FLOOR: f32 = 1e-4;
/// Bit depth the crusher quantises to.
const CRUSH_BITS: u32 = 5;
/// Sample rate reduction: one sample is held for this many frames.
const CRUSH_HOLD: usize = 24;

/// The units the slot can run. `next` cycles in this order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FxKind {
    /// Tape-style feedback delay, timed in beats.
    #[default]
    Echo,
    /// Sample rate and bit depth reduction.
    Bitcrusher,
}

impl FxKind {
    pub fn next(self) -> Self {
        match self {
            Self::Echo => Self::Bitcrusher,
            Self::Bitcrusher => Self::Echo,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Echo => "Echo",
            Self::Bitcrusher => "Bitcrusher",
        }
    }
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
    fn process(&mut self, buf: &mut [f32], delay_frames: usize, feed: &mut Ramp) {
        let frames = self.line.len() / 2;
        let delay = delay_frames.clamp(1, frames - 1);
        for frame in buf.as_chunks_mut::<2>().0 {
            let read = (self.write + frames - delay) % frames;
            let (l, r) = (self.line[read * 2], self.line[read * 2 + 1]);
            let feed = feed.next();
            let (wl, wr) = (
                frame[0] * feed + l * ECHO_FEEDBACK as f32,
                frame[1] * feed + r * ECHO_FEEDBACK as f32,
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

/// Sample rate reduction with bit depth reduction on top.
#[derive(Default)]
struct Bitcrusher {
    held: [f32; 2],
    counter: usize,
}

impl Bitcrusher {
    fn process(&mut self, buf: &mut [f32]) {
        let levels = ((1u32 << CRUSH_BITS) - 1) as f32;
        for frame in buf.as_chunks_mut::<2>().0 {
            if self.counter == 0 {
                self.held = [
                    (frame[0].clamp(-1.0, 1.0) * levels).round() / levels,
                    (frame[1].clamp(-1.0, 1.0) * levels).round() / levels,
                ];
            }
            self.counter = (self.counter + 1) % CRUSH_HOLD;
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
    echo: Echo,
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
            echo: Echo::new(fs),
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

    /// True while a unit still has something to play after being switched off.
    pub fn is_ringing(&self) -> bool {
        !self.on && matches!(self.kind, FxKind::Echo) && self.echo.rings()
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
        match self.kind {
            FxKind::Echo => {
                let delay = (ECHO_BEATS * self.beat_frames as f64) as usize;
                self.echo.process(buf, delay, &mut self.feed);
            }
            FxKind::Bitcrusher => self.crusher.process(buf),
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
