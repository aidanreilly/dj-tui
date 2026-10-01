use crate::{best_match, parse_search};
use std::time::Duration;

/// Discogs allows 60 requests a minute to an authenticated caller. A second between them
/// stays under that without tracking a window.
pub const MIN_REQUEST_GAP: Duration = Duration::from_secs(1);

/// Discogs asks every caller to identify itself.
const USER_AGENT: &str = concat!("dj-tui/", env!("CARGO_PKG_VERSION"));

#[derive(Debug)]
pub enum Error {
    /// The token is missing or wrong.
    Unauthorized,
    /// Too many requests.
    RateLimited,
    /// Offline, DNS, a timeout, or a server that is having a bad day.
    Network(String),
    Parse(String),
}

impl Error {
    /// True when the next call would fail the same way, so a batch pass should stop rather
    /// than work through the rest of the library finding out.
    pub fn stops_the_pass(&self) -> bool {
        matches!(self, Error::Unauthorized | Error::RateLimited)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unauthorized => write!(f, "Discogs rejected the token"),
            Error::RateLimited => write!(f, "Discogs rate limit reached"),
            Error::Network(e) => write!(f, "could not reach Discogs: {e}"),
            Error::Parse(e) => write!(f, "could not read what Discogs sent: {e}"),
        }
    }
}

impl std::error::Error for Error {}

pub struct Client {
    token: String,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(token: String) -> Client {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .build();
        Client {
            token,
            agent: config.new_agent(),
        }
    }

    /// The error a status code means, or `None` when it is not one.
    pub fn error_for_status(status: u16) -> Option<Error> {
        match status {
            401 | 403 => Some(Error::Unauthorized),
            429 => Some(Error::RateLimited),
            200..=299 => None,
            other => Some(Error::Network(format!("HTTP {other}"))),
        }
    }

    /// Search Discogs for this record and take the genre off it, or `None` when nothing
    /// there is confidently the same record.
    pub fn genre(&self, artist: &str, title: &str) -> Result<Option<String>, Error> {
        let response = self
            .agent
            .get("https://api.discogs.com/database/search")
            .query("artist", artist)
            .query("track", title)
            .query("type", "release")
            .header("User-Agent", USER_AGENT)
            .header("Authorization", &format!("Discogs token={}", self.token))
            .call();
        let body = match response {
            Ok(mut r) => r
                .body_mut()
                .read_to_string()
                .map_err(|e| Error::Network(e.to_string()))?,
            Err(ureq::Error::StatusCode(code)) => {
                return Err(Self::error_for_status(code)
                    .unwrap_or_else(|| Error::Network(format!("HTTP {code}"))))
            }
            Err(e) => return Err(Error::Network(e.to_string())),
        };
        let results = parse_search(&body).map_err(|e| Error::Parse(e.to_string()))?;
        Ok(best_match(&results, artist, title).and_then(|r| r.genre()))
    }
}
