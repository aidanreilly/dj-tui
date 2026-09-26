//! Waveform analysis, the point type it produces, and the on-disk cache for both.
//!
//! No audio decoding and no rendering live here, so `loader` and `tui` can both
//! depend on it.

mod biquad;

pub use biquad::{Biquad, Q};
