//! What the client does with a response, without making one. The live call is behind an
//! environment variable, following DJ_TUI_ALSA_TESTS and DJ_TUI_JACK_TESTS.

use discogs::{Client, Error};

#[test]
fn a_401_says_the_token_is_wrong_rather_than_that_there_is_no_genre() {
    // A wrong token would otherwise look exactly like a library full of unknown records.
    assert!(matches!(
        Client::error_for_status(401),
        Some(Error::Unauthorized)
    ));
    assert!(matches!(
        Client::error_for_status(403),
        Some(Error::Unauthorized)
    ));
}

#[test]
fn a_429_says_it_is_rate_limited() {
    assert!(matches!(
        Client::error_for_status(429),
        Some(Error::RateLimited)
    ));
}

#[test]
fn a_200_is_not_an_error() {
    assert!(Client::error_for_status(200).is_none());
}

#[test]
fn another_status_is_reported_as_itself() {
    assert!(matches!(
        Client::error_for_status(503),
        Some(Error::Network(_))
    ));
}

#[test]
fn requests_are_spaced_so_the_rate_limit_is_not_reached() {
    // Discogs allows 60 a minute to an authenticated caller.
    assert!(
        discogs::MIN_REQUEST_GAP >= std::time::Duration::from_secs(1),
        "a gap of {:?} would run into the limit",
        discogs::MIN_REQUEST_GAP
    );
}

#[test]
fn an_unauthorized_or_rate_limited_error_says_the_whole_pass_should_stop() {
    // One is a wrong token and the other is too many requests. Both mean the next call
    // fails the same way, so a batch that keeps going just burns the library.
    assert!(Error::Unauthorized.stops_the_pass());
    assert!(Error::RateLimited.stops_the_pass());
    assert!(!Error::Network("offline".into()).stops_the_pass());
}

#[test]
fn a_real_lookup_finds_a_known_record() {
    if std::env::var("DJ_TUI_DISCOGS_TESTS").as_deref() != Ok("1") {
        return;
    }
    let token = std::env::var("DJ_TUI_DISCOGS_TOKEN").expect("a token");
    let client = Client::new(token);
    let genre = client.genre("Objekt", "Cactus").expect("the call works");
    assert!(genre.is_some(), "a well known record has a genre");
}
