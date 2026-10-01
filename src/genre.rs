//! The genre lookup the loader asks, implemented over the Discogs client.
//!
//! `loader` names what it needs as a trait so that decoding audio does not drag in an HTTP
//! client and a TLS stack for every consumer of the crate. This is the one place the two
//! meet, and it is also where the cache of earlier answers lives.

use loader::{GenreLookup, LookupError};
use std::time::Duration;

pub struct Discogs {
    client: discogs::Client,
}

impl Discogs {
    /// `None` unless the lookup is switched on in the config and a token is in the
    /// environment. The cache comes from the state folder, so a question answered on an
    /// earlier run is not asked again.
    pub fn new(enabled: bool) -> Option<Discogs> {
        let token = std::env::var("DJ_TUI_DISCOGS_TOKEN")
            .ok()
            .filter(|t| !t.is_empty())?;
        if !enabled {
            return None;
        }
        let client = match crate::discogs_cache_path(
            std::env::var("XDG_STATE_HOME").ok().as_deref(),
            std::env::var("HOME").ok().as_deref(),
        ) {
            Some(path) => discogs::Client::with_cache(token, discogs::Cache::load(&path)),
            None => discogs::Client::new(token),
        };
        Some(Discogs { client })
    }
}

impl GenreLookup for Discogs {
    fn genre(&self, artist: &str, title: &str) -> Result<Option<String>, LookupError> {
        self.client.genre(artist, title).map_err(|e| LookupError {
            message: e.to_string(),
            fatal: e.stops_the_pass(),
        })
    }

    fn knows(&self, artist: &str, title: &str) -> bool {
        self.client.knows(artist, title)
    }

    fn request_gap(&self) -> Duration {
        discogs::MIN_REQUEST_GAP
    }

    fn remember(&self) {
        self.client.save_cache();
    }
}
