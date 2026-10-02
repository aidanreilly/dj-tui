//! Real-time audio engine. No I/O lives here, so every block can be driven offline in tests.

mod deck;
pub mod dsp;
mod mixer;
mod rt;
mod stretch;
mod track;

pub use deck::{Deck, HOT_CUES};
pub use mixer::{
    crossfader_gains, CrossfaderCurve, DeckId, Engine, Meters, OutputMode, MAX_BLOCK_FRAMES,
};
pub use rt::{channel, Command, DeckSnapshot, EngineHandle, EngineProcessor, Snapshot};
pub use track::Track;
