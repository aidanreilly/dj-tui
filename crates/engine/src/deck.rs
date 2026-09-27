use crate::Track;
use std::sync::Arc;

pub const HOT_CUES: usize = 8;

/// One playback deck: transport, main cue and hot cues. Positions are in frames.
#[derive(Default)]
pub struct Deck {
    track: Option<Arc<Track>>,
    pos: f64,
    rate: f64,
    playing: bool,
    /// True while the main cue is held from a paused state.
    previewing: bool,
    cue_point: f64,
    hot_cues: [Option<f64>; HOT_CUES],
}

impl Deck {
    pub fn new() -> Self {
        Self {
            rate: 1.0,
            ..Default::default()
        }
    }

    /// Load `track`, resetting transport and cues. Returns the previous track so the caller
    /// decides where it gets freed (never on the audio thread).
    pub fn load(&mut self, track: Arc<Track>) -> Option<Arc<Track>> {
        let old = std::mem::replace(
            self,
            Self {
                rate: self.rate,
                ..Default::default()
            },
        );
        self.track = Some(track);
        old.track
    }

    pub fn cue_is_previewing(&self) -> bool {
        self.previewing
    }

    pub fn track(&self) -> Option<&Arc<Track>> {
        self.track.as_ref()
    }

    pub fn play_pause(&mut self) {
        if self.track.is_none() {
            return;
        }
        if self.previewing {
            // Play during a cue preview latches playback; releasing cue no longer snaps back.
            self.previewing = false;
            return;
        }
        self.playing = !self.playing;
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn position(&self) -> f64 {
        self.pos
    }

    pub fn seek(&mut self, frame: f64) {
        let len = self.len();
        self.pos = frame.clamp(0.0, len);
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    pub fn set_rate(&mut self, rate: f64) {
        self.rate = rate;
    }

    /// Render into interleaved stereo `out`. Allocation free; safe for the audio thread.
    pub fn render(&mut self, out: &mut [f32]) {
        let Some(track) = self.track.as_deref() else {
            out.fill(0.0);
            return;
        };
        let len = track.frames() as f64;
        for frame in out.as_chunks_mut::<2>().0 {
            if !self.playing {
                frame.fill(0.0);
                continue;
            }
            if self.pos >= len {
                self.pos = len;
                self.playing = false;
                frame.fill(0.0);
                continue;
            }
            let (l, r) = track.frame_at(self.pos);
            frame[0] = l;
            frame[1] = r;
            self.pos += self.rate;
        }
    }

    pub fn cue_press(&mut self) {
        if self.track.is_none() {
            return;
        }
        if self.playing && !self.previewing {
            self.pos = self.cue_point;
            self.playing = false;
        } else if !self.playing {
            self.cue_point = self.pos;
            self.previewing = true;
            self.playing = true;
        }
    }

    pub fn cue_release(&mut self) {
        if self.previewing {
            self.previewing = false;
            self.playing = false;
            self.pos = self.cue_point;
        }
    }

    pub fn set_cue_point(&mut self, frame: f64) {
        self.cue_point = frame.clamp(0.0, self.len());
    }

    /// Put hot cue `n` at `frame`, or clear it with `None`. Out of range pads are ignored.
    pub fn set_hot_cue(&mut self, n: usize, frame: Option<f64>) {
        let len = self.len();
        if let Some(slot) = self.hot_cues.get_mut(n) {
            *slot = frame.map(|f| f.clamp(0.0, len));
        }
    }

    pub fn cue_point(&self) -> f64 {
        self.cue_point
    }

    /// Jump to hot cue `n` if set, otherwise store the current position there.
    pub fn hot_cue(&mut self, n: usize) {
        if self.track.is_none() || n >= HOT_CUES {
            return;
        }
        match self.hot_cues[n] {
            Some(p) => self.pos = p,
            None => self.hot_cues[n] = Some(self.pos),
        }
    }

    pub fn clear_hot_cue(&mut self, n: usize) {
        if let Some(slot) = self.hot_cues.get_mut(n) {
            *slot = None;
        }
    }

    pub fn hot_cue_position(&self, n: usize) -> Option<f64> {
        self.hot_cues.get(n).copied().flatten()
    }

    fn len(&self) -> f64 {
        self.track.as_ref().map_or(0.0, |t| t.frames() as f64)
    }
}
