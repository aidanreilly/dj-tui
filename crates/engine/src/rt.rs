//! The boundary between the UI and the audio thread.
//!
//! Commands travel over a wait-free SPSC ring. State comes back as atomics written once per
//! `process` call. Tracks replaced by a `Load` go back over a second ring so their memory is
//! freed on the UI thread.

use crate::{CrossfaderCurve, DeckId, Engine, Track, HOT_CUES};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::Arc;

#[derive(Debug)]
pub enum Command {
    Load(DeckId, Arc<Track>),
    PlayPause(DeckId),
    CuePress(DeckId),
    CueRelease(DeckId),
    HotCue(DeckId, usize),
    ClearHotCue(DeckId, usize),
    Seek(DeckId, f64),
    SetRate(DeckId, f64),
    SetChannelFader(DeckId, f32),
    SetCrossfader(f32),
    SetCrossfaderCurve(CrossfaderCurve),
    SetHeadphoneCue(DeckId, bool),
    SetCueMix(f32),
    SetTrim(DeckId, f32),
    SetEq(DeckId, crate::dsp::EqBand, f32),
    SetEqKill(DeckId, crate::dsp::EqBand, bool),
    SetFilter(DeckId, f32),
    /// Place the main cue at a frame, as when restoring saved cues.
    SetCuePoint(DeckId, f64),
    /// Place or clear hot cue `n` at a frame without moving the playhead.
    SetHotCue(DeckId, usize, Option<f64>),
    /// Loop between two frames, or clear the loop with `None`.
    SetLoop(DeckId, Option<(f64, f64)>),
    /// Effect slot: which unit, whether it runs, and how much of it is heard.
    SetFxKind(DeckId, crate::fx::FxKind),
    SetFxOn(DeckId, bool),
    SetFxWet(DeckId, f32),
    /// Beat length of the loaded track in frames, which times the tempo-aware effects.
    SetBeatFrames(DeckId, f32),
    /// Hold the pitch while the tempo fader moves.
    SetKeyLock(DeckId, bool),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeckSnapshot {
    pub position: f64,
    pub rate: f64,
    pub playing: bool,
    pub cue_point: f64,
    pub hot_cues: [Option<f64>; HOT_CUES],
    pub track_frames: usize,
    /// Active loop as (start, end) in frames.
    pub loop_span: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub decks: [DeckSnapshot; 2],
    pub crossfader: f32,
    pub faders: [f32; 2],
    pub headphone_cue: [bool; 2],
    pub crossfader_curve: CrossfaderCurve,
    pub frames_processed: u64,
}

#[derive(Default)]
struct F64(AtomicU64);
impl F64 {
    fn set(&self, v: f64) {
        self.0.store(v.to_bits(), Relaxed)
    }
    fn get(&self) -> f64 {
        f64::from_bits(self.0.load(Relaxed))
    }
}

#[derive(Default)]
struct F32(AtomicU32);
impl F32 {
    fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Relaxed)
    }
    fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Relaxed))
    }
}

#[derive(Default)]
struct SharedDeck {
    position: F64,
    rate: F64,
    playing: AtomicBool,
    cue_point: F64,
    /// NaN encodes an empty slot.
    hot_cues: [F64; HOT_CUES],
    /// Both NaN when no loop is active. The pair can be read a frame apart from the position
    /// above, which only moves a loop marker in the UI by one frame.
    loop_span: [F64; 2],
    track_frames: AtomicUsize,
    fader: F32,
    headphone_cue: AtomicBool,
}

#[derive(Default)]
struct Shared {
    /// Peak levels as f32 bits. Non-negative floats order the same as their bits, so
    /// `fetch_max` accumulates peaks between UI reads.
    meters: [AtomicU32; 3],
    decks: [SharedDeck; 2],
    crossfader: F32,
    curve: std::sync::atomic::AtomicU8,
    frames_processed: AtomicU64,
}

/// UI-side end: sends commands, reads snapshots, frees replaced tracks.
pub struct EngineHandle {
    commands: Producer<Command>,
    garbage: Consumer<Arc<Track>>,
    shared: Arc<Shared>,
}

/// Audio-side end: owns the engine. `process` is allocation and lock free.
pub struct EngineProcessor {
    engine: Engine,
    commands: Consumer<Command>,
    garbage: Producer<Arc<Track>>,
    shared: Arc<Shared>,
    frames_processed: u64,
}

/// Split an engine into a UI handle and an audio processor. `capacity` bounds queued commands.
pub fn channel(engine: Engine, capacity: usize) -> (EngineHandle, EngineProcessor) {
    let (ctx, crx) = RingBuffer::new(capacity);
    // Every command could be a Load, so the garbage ring can never overflow before the
    // command ring does.
    let (gtx, grx) = RingBuffer::new(capacity * 2);
    let shared = Arc::new(Shared::default());
    let mut processor = EngineProcessor {
        engine,
        commands: crx,
        garbage: gtx,
        shared: shared.clone(),
        frames_processed: 0,
    };
    processor.publish();
    (
        EngineHandle {
            commands: ctx,
            garbage: grx,
            shared,
        },
        processor,
    )
}

impl EngineHandle {
    /// Queue a command. Gives it back if the queue is full.
    pub fn send(&mut self, cmd: Command) -> Result<(), Command> {
        self.commands
            .push(cmd)
            .map_err(|rtrb::PushError::Full(c)| c)
    }

    pub fn snapshot(&self) -> Snapshot {
        let s = &*self.shared;
        let deck = |d: &SharedDeck| {
            let mut hot_cues = [None; HOT_CUES];
            for (slot, v) in hot_cues.iter_mut().zip(&d.hot_cues) {
                let p = v.get();
                *slot = (!p.is_nan()).then_some(p);
            }
            DeckSnapshot {
                position: d.position.get(),
                rate: d.rate.get(),
                playing: d.playing.load(Relaxed),
                cue_point: d.cue_point.get(),
                hot_cues,
                track_frames: d.track_frames.load(Relaxed),
                loop_span: {
                    let (start, end) = (d.loop_span[0].get(), d.loop_span[1].get());
                    (!start.is_nan() && !end.is_nan()).then_some((start, end))
                },
            }
        };
        Snapshot {
            decks: [deck(&s.decks[0]), deck(&s.decks[1])],
            crossfader: s.crossfader.get(),
            faders: [s.decks[0].fader.get(), s.decks[1].fader.get()],
            headphone_cue: [
                s.decks[0].headphone_cue.load(Relaxed),
                s.decks[1].headphone_cue.load(Relaxed),
            ],
            crossfader_curve: match s.curve.load(Relaxed) {
                0 => CrossfaderCurve::Linear,
                2 => CrossfaderCurve::Cut,
                _ => CrossfaderCurve::ConstantPower,
            },
            frames_processed: s.frames_processed.load(Relaxed),
        }
    }

    /// Peak levels since the previous call. Resets them.
    pub fn take_meters(&self) -> crate::Meters {
        let m = &self.shared.meters;
        let take = |i: usize| f32::from_bits(m[i].swap(0, Relaxed));
        crate::Meters {
            channels: [take(0), take(1)],
            master: take(2),
        }
    }

    /// Free tracks the audio thread has let go of. Call regularly from the UI loop.
    pub fn collect_garbage(&mut self) -> usize {
        let mut n = 0;
        while self.garbage.pop().is_ok() {
            n += 1;
        }
        n
    }
}

impl EngineProcessor {
    pub fn process(&mut self, master: &mut [f32], cue: &mut [f32]) {
        while let Ok(cmd) = self.commands.pop() {
            if let Some(old) = self.engine.apply(cmd) {
                if let Err(rtrb::PushError::Full(old)) = self.garbage.push(old) {
                    // Unreachable given the ring sizes; leaking beats freeing on this thread.
                    std::mem::forget(old);
                }
            }
        }
        self.engine.process(master, cue);
        let peaks = self.engine.take_peaks();
        let m = &self.shared.meters;
        m[0].fetch_max(peaks.channels[0].to_bits(), Relaxed);
        m[1].fetch_max(peaks.channels[1].to_bits(), Relaxed);
        m[2].fetch_max(peaks.master.to_bits(), Relaxed);
        self.frames_processed += (master.len() / 2) as u64;
        self.publish();
    }

    /// Beat length the deck's effect slot is timing to, after the tempo fader.
    pub fn fx_beat_frames(&self, id: DeckId) -> f32 {
        self.engine.fx_beat_frames(id)
    }

    /// Size of the internal command backlog, for diagnostics.
    pub fn pending_commands(&self) -> usize {
        self.commands.slots()
    }

    fn publish(&mut self) {
        let s = &*self.shared;
        for id in [DeckId::A, DeckId::B] {
            let deck = self.engine.deck(id);
            let out = &s.decks[id.index()];
            out.position.set(deck.position());
            out.rate.set(deck.rate());
            out.playing.store(deck.is_playing(), Relaxed);
            out.cue_point.set(deck.cue_point());
            for (i, slot) in out.hot_cues.iter().enumerate() {
                slot.set(deck.hot_cue_position(i).unwrap_or(f64::NAN));
            }
            let span = deck.loop_span();
            out.loop_span[0].set(span.map_or(f64::NAN, |(start, _)| start));
            out.loop_span[1].set(span.map_or(f64::NAN, |(_, end)| end));
            out.track_frames
                .store(deck.track().map_or(0, |t| t.frames()), Relaxed);
            out.fader.set(self.engine.channel_fader(id));
            out.headphone_cue
                .store(self.engine.headphone_cue(id), Relaxed);
        }
        s.crossfader.set(self.engine.crossfader());
        s.curve.store(
            match self.engine.crossfader_curve() {
                CrossfaderCurve::Linear => 0,
                CrossfaderCurve::ConstantPower => 1,
                CrossfaderCurve::Cut => 2,
            },
            Relaxed,
        );
        s.frames_processed.store(self.frames_processed, Relaxed);
    }
}
