//! Real-time audio engine. No I/O lives here, so every block can be driven offline in tests.

mod deck;
mod mixer;
mod track;

pub use deck::{Deck, HOT_CUES};
pub use mixer::{crossfader_gains, CrossfaderCurve, DeckId, Engine};
pub use track::Track;
