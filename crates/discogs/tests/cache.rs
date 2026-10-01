//! The lookup cache: what has already been asked, so nothing is asked twice.

use discogs::Cache;

#[test]
fn a_hit_comes_back_without_going_to_the_network() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("discogs.json");
    let mut cache = Cache::load(&path);
    assert_eq!(cache.get("Objekt", "Cactus"), None, "nothing asked yet");

    cache.insert("Objekt", "Cactus", Some("Techno".into()));
    assert_eq!(
        cache.get("Objekt", "Cactus"),
        Some(Some("Techno".to_string()))
    );
}

#[test]
fn a_miss_is_remembered_as_a_miss() {
    // Asked and nothing matched is an answer. Storing it as "not asked" would send the same
    // fruitless request on every run.
    let dir = tempfile::tempdir().unwrap();
    let mut cache = Cache::load(&dir.path().join("discogs.json"));
    cache.insert("Nobody", "Nothing", None);
    assert_eq!(cache.get("Nobody", "Nothing"), Some(None));
}

#[test]
fn it_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("discogs.json");
    let mut cache = Cache::load(&path);
    cache.insert("Objekt", "Cactus", Some("Techno".into()));
    cache.save().expect("it writes");

    let read = Cache::load(&path);
    assert_eq!(
        read.get("Objekt", "Cactus"),
        Some(Some("Techno".to_string()))
    );
}

#[test]
fn the_key_ignores_case_and_punctuation_so_a_retag_still_hits() {
    // The same record ripped twice, tagged slightly differently, is one question.
    let dir = tempfile::tempdir().unwrap();
    let mut cache = Cache::load(&dir.path().join("discogs.json"));
    cache.insert("Objekt", "Cactus", Some("Techno".into()));
    assert_eq!(
        cache.get("objekt!", "  CACTUS "),
        Some(Some("Techno".to_string()))
    );
}

#[test]
fn a_missing_or_corrupt_file_is_an_empty_cache_rather_than_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(Cache::load(&dir.path().join("none.json")).len(), 0);
    let bad = dir.path().join("bad.json");
    std::fs::write(&bad, b"this is not json").unwrap();
    assert_eq!(Cache::load(&bad).len(), 0, "a bad file is not a crash");
}

#[test]
fn saving_creates_the_directory_it_needs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state").join("dj-tui").join("discogs.json");
    let mut cache = Cache::load(&path);
    cache.insert("Objekt", "Cactus", Some("Techno".into()));
    cache.save().expect("it makes the folder");
    assert!(path.is_file());
}

#[test]
fn a_client_with_a_cached_answer_does_not_call_out() {
    // No token and no network here. If the client reached for either, this would fail
    // rather than return the stored answer.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("discogs.json");
    let mut cache = Cache::load(&path);
    cache.insert("Objekt", "Cactus", Some("Techno".into()));
    cache.insert("Nobody", "Nothing", None);
    cache.save().unwrap();

    let client = discogs::Client::with_cache(String::new(), Cache::load(&path));
    assert_eq!(
        client.genre("Objekt", "Cactus").unwrap(),
        Some("Techno".to_string())
    );
    assert_eq!(
        client.genre("nobody!", "NOTHING").unwrap(),
        None,
        "a remembered miss is an answer, not a reason to ask again"
    );
}
