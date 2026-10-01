//! Genre lookup against Discogs: artist and title in, a genre string or nothing out.
//!
//! The parsing and the matching are pure and are what the tests exercise. The network sits
//! behind `Client` and the suite never touches it, which follows `crates/midi`: the mapping
//! layer is tested from strings and the hardware lives at the edge.
//!
//! Nothing here looks at audio. A genre is what a catalogue entry says, matched on the
//! artist and title a file's own tags gave.

mod client;
mod release;

pub use client::{Client, Error, MIN_REQUEST_GAP};
pub use release::{best_match, parse_search, Release};
