//! Which colour scheme the waveform draws in. Cycled with `W`, set by `[ui] waveform_mode`.

/// The three schemes `docs/spec.md` takes from the CDJ-3000 and rekordbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub enum WaveformMode {
    /// Bands overlaid at their own heights: bass outermost, highs as a core at the centre.
    #[default]
    #[cfg_attr(feature = "serde", serde(rename = "3band"))]
    ThreeBand,
    /// One blended colour per column, low to red, mid to green, high to blue.
    #[cfg_attr(feature = "serde", serde(rename = "rgb"))]
    Rgb,
    /// A single blue, tinting toward white as high content rises.
    #[cfg_attr(feature = "serde", serde(rename = "blue"))]
    Blue,
}

impl WaveformMode {
    pub fn next(self) -> Self {
        match self {
            Self::ThreeBand => Self::Rgb,
            Self::Rgb => Self::Blue,
            Self::Blue => Self::ThreeBand,
        }
    }
}
