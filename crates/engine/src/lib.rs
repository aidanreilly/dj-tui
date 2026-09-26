//! Real-time audio engine. No I/O lives here, so every block can be driven offline in tests.

mod deck;
mod mixer;
mod rt;
mod track;

pub use deck::{Deck, HOT_CUES};
pub use mixer::{crossfader_gains, CrossfaderCurve, DeckId, Engine, MAX_BLOCK_FRAMES};
pub use rt::{channel, Command, DeckSnapshot, EngineHandle, EngineProcessor, Snapshot};
pub use track::Track;
