use engine::EngineProcessor;

/// Renders the engine's interleaved master and cue output into four planar port buffers.
/// Scratch space is reserved up front; `render` never allocates.
pub struct PlanarRenderer {
    master: Vec<f32>,
    cue: Vec<f32>,
    frames_rendered: u64,
}

impl PlanarRenderer {
    pub fn new(max_frames: usize) -> Self {
        Self {
            master: vec![0.0; max_frames * 2],
            cue: vec![0.0; max_frames * 2],
            frames_rendered: 0,
        }
    }

    /// Grow scratch space. Call from a non-real-time context such as JACK's buffer size callback.
    pub fn reserve(&mut self, max_frames: usize) {
        if self.master.len() < max_frames * 2 {
            self.master.resize(max_frames * 2, 0.0);
            self.cue.resize(max_frames * 2, 0.0);
        }
    }

    /// `outs` is master L, master R, cue L, cue R, all the same length.
    pub fn render(&mut self, processor: &mut EngineProcessor, outs: [&mut [f32]; 4]) {
        let n = outs[0].len();
        if n * 2 > self.master.len() {
            // Buffer grew without a reserve() call: output silence rather than allocate here.
            for o in outs {
                o.fill(0.0);
            }
            return;
        }
        let (m, c) = (&mut self.master[..n * 2], &mut self.cue[..n * 2]);
        processor.process(m, c);
        let [ml, mr, cl, cr] = outs;
        for i in 0..n {
            ml[i] = m[i * 2];
            mr[i] = m[i * 2 + 1];
            cl[i] = c[i * 2];
            cr[i] = c[i * 2 + 1];
        }
        self.frames_rendered += n as u64;
    }

    pub fn frames_rendered(&self) -> u64 {
        self.frames_rendered
    }
}
