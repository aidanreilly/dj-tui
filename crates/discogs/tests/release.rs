//! Reading a Discogs search response, and deciding whether what came back is the record
//! that was asked for. Nothing here touches the network.

use discogs::{best_match, parse_search};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("tests/fixtures/{name}")).expect("a fixture")
}

#[test]
fn styles_are_preferred_over_the_genre() {
    // Discogs calls a techno twelve inch "Electronic". The styles on the same release are
    // what a filter is worth having.
    let results = parse_search(&fixture("search-objekt-cactus.json")).unwrap();
    assert_eq!(results[0].genre().as_deref(), Some("Techno, Breakbeat"));
}

#[test]
fn a_release_with_no_style_falls_back_to_its_genre() {
    let results = parse_search(&fixture("no-style.json")).unwrap();
    assert_eq!(results[0].genre().as_deref(), Some("Electronic"));
}

#[test]
fn a_release_with_neither_has_no_genre_at_all() {
    // An empty string here would be stored and would then filter as a real genre.
    let results = parse_search(&fixture("no-style.json")).unwrap();
    assert_eq!(results[1].genre(), None);
}

#[test]
fn a_missing_results_key_is_an_empty_list_rather_than_an_error() {
    assert!(parse_search("{}").unwrap().is_empty());
}

#[test]
fn best_match_takes_the_record_that_was_asked_for() {
    let results = parse_search(&fixture("search-objekt-cactus.json")).unwrap();
    let got = best_match(&results, "Objekt", "Cactus").expect("the right release");
    assert_eq!(got.genre().as_deref(), Some("Techno, Breakbeat"));
}

#[test]
fn best_match_takes_nothing_when_only_the_artist_matches() {
    // A different record by the same artist is the wrong answer, and a wrong genre filters a
    // track out of the list it belonged in.
    let results = parse_search(&fixture("search-objekt-cactus.json")).unwrap();
    assert!(best_match(&results, "Objekt", "Theme From Q").is_none());
}

#[test]
fn best_match_takes_nothing_when_only_the_title_matches() {
    let results = parse_search(&fixture("search-objekt-cactus.json")).unwrap();
    assert!(best_match(&results, "Someone Else", "Cactus").is_none());
}

#[test]
fn matching_folds_case_punctuation_and_feature_credits() {
    let results = parse_search(&fixture("search-objekt-cactus.json")).unwrap();
    assert!(best_match(&results, "objekt!", "cactus (feat. nobody)").is_some());
}

#[test]
fn nothing_matches_an_empty_artist_or_title() {
    // A file with no tags at all gives these. Everything would contain an empty string.
    let results = parse_search(&fixture("search-objekt-cactus.json")).unwrap();
    assert!(best_match(&results, "", "Cactus").is_none());
    assert!(best_match(&results, "Objekt", "").is_none());
}
