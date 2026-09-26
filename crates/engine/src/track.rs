/// A fully decoded track held in memory as interleaved stereo f32 at the session rate.
pub struct Track {
    data: Vec<f32>,
    sample_rate: u32,
}

impl std::fmt::Debug for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Track")
            .field("frames", &self.frames())
            .field("sample_rate", &self.sample_rate)
            .finish()
    }
}

impl Track {
    /// `data` must be interleaved stereo; a trailing odd sample is dropped.
    pub fn from_interleaved(mut data: Vec<f32>, sample_rate: u32) -> Self {
        data.truncate(data.len() & !1);
        Self { data, sample_rate }
    }

    pub fn frames(&self) -> usize {
        self.data.len() / 2
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Linearly interpolated stereo frame at a fractional position. Out of range reads are silent.
    #[inline]
    pub fn frame_at(&self, pos: f64) -> (f32, f32) {
        if pos < 0.0 {
            return (0.0, 0.0);
        }
        let i = pos as usize;
        let frames = self.frames();
        if i >= frames {
            return (0.0, 0.0);
        }
        let frac = (pos - i as f64) as f32;
        let (l0, r0) = (self.data[i * 2], self.data[i * 2 + 1]);
        if frac == 0.0 || i + 1 >= frames {
            return (l0, r0);
        }
        let (l1, r1) = (self.data[i * 2 + 2], self.data[i * 2 + 3]);
        (l0 + (l1 - l0) * frac, r0 + (r1 - r0) * frac)
    }
}
