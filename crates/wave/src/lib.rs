//! Waveform analysis, the point type it produces, and the on-disk cache for both.
//!
//! No audio decoding and no rendering live here, so `loader` and `tui` can both
//! depend on it.

mod analysis;
mod biquad;
pub mod cache;
mod mode;

pub use analysis::{analyse, WavePoint, HIGH_HZ, LOW_HZ, POINTS_PER_SECOND};
pub use biquad::{Biquad, Q};
pub use mode::WaveformMode;
