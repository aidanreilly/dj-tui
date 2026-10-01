//! Scanning a music folder, searching it, sorting it, and spotting a compatible key.

use analysis::key::Key;
use library::{compatible, scan, search, sort, Column, Entry, Playlist, Query};
use std::path::Path;

fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"not really audio").unwrap();
}

fn names(entries: &[Entry]) -> Vec<String> {
    entries.iter().map(|e| e.display().to_string()).collect()
}

#[test]
fn a_scan_finds_audio_and_leaves_everything_else_alone() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "Warehouse Tool.flac",
        "b/Breakdown Edit.mp3",
        "b/c/Late Night.wav",
        "notes.txt",
        "cover.jpg",
        "Warehouse Tool.flac.dj-tui.json",
    ] {
        touch(&dir.path().join(name));
    }
    let found = scan(&[dir.path().to_path_buf()]);
    assert_eq!(
        names(&found),
        vec!["Breakdown Edit", "Late Night", "Warehouse Tool"],
        "sorted by name, sidecars and artwork ignored"
    );
    assert!(found.iter().all(|e| !e.analysed()), "nothing analysed yet");
}

#[test]
fn a_scan_of_nowhere_is_empty_rather_than_an_error() {
    assert!(scan(&[Path::new("/definitely/not/here").to_path_buf()]).is_empty());
    assert!(scan(&[]).is_empty());
}

#[test]
fn what_the_sidecar_knows_fills_the_columns() {
    let dir = tempfile::tempdir().unwrap();
    let audio = dir.path().join("Warehouse Tool.wav");
    touch(&audio);
    let sidecar = r#"{"format":"dj-tui track data","version":1,
            "audio":{"file_size":16,"fingerprint":"0000000000000000"},
            "analysis":{"bpm":128.0,"first_beat_secs":0.0,"key":"8A","key_name":"A minor"},
            "waveform":{"points":0,"ranges":[],"bands":[]},
            "track":{"title":"Warehouse Tool","artist":"Synthetic","duration_secs":312.0}}"#;
    std::fs::write(loader::sidecar::sidecar_path(&audio), sidecar).unwrap();

    let found = scan(&[dir.path().to_path_buf()]);
    let entry = &found[0];
    assert_eq!(entry.display(), "Synthetic - Warehouse Tool");
    assert_eq!(entry.bpm(), Some(128.0));
    assert_eq!(entry.key(), Key::from_camelot("8A"));
    assert_eq!(entry.duration_secs(), Some(312.0));
    assert!(entry.analysed(), "it has been through analysis");
}

#[test]
fn a_track_whose_tempo_came_back_empty_is_still_waiting_for_analysis() {
    let dir = tempfile::tempdir().unwrap();
    let audio = dir.path().join("Tape Transfer.wav");
    touch(&audio);
    let sidecar = r#"{"format":"dj-tui track data","version":1,
            "audio":{"file_size":16,"fingerprint":"0000000000000000"},
            "analysis":{"bpm":null,"first_beat_secs":null,"key":"8A","key_name":"A minor"},
            "waveform":{"points":0,"ranges":[],"bands":[]},
            "track":{"title":"Tape Transfer","artist":"Synthetic","duration_secs":312.0}}"#;
    std::fs::write(loader::sidecar::sidecar_path(&audio), sidecar).unwrap();

    let entry = &scan(&[dir.path().to_path_buf()])[0];
    assert_eq!(entry.key(), Key::from_camelot("8A"), "the key was found");
    assert!(
        !entry.analysed(),
        "no tempo, so a batch run should pick it up again"
    );
}

/// name, BPM, Camelot key, length.
type Spec<'a> = (&'a str, Option<f64>, Option<&'a str>, Option<f64>);

fn entries(specs: &[Spec]) -> Vec<Entry> {
    specs
        .iter()
        .map(|(name, bpm, key, secs)| {
            Entry::for_test(
                Path::new(name),
                name.trim_end_matches(".wav").to_string(),
                *bpm,
                key.and_then(Key::from_camelot),
                *secs,
                None,
            )
        })
        .collect()
}

#[test]
fn columns_sort_both_ways_and_unknowns_sink() {
    let mut list = entries(&[
        ("Beta.wav", Some(128.0), Some("8A"), Some(300.0)),
        ("alpha.wav", Some(124.0), Some("9A"), Some(200.0)),
        ("Gamma.wav", None, None, None),
    ]);

    sort(&mut list, Column::Name, true);
    assert_eq!(
        names(&list),
        vec!["alpha", "Beta", "Gamma"],
        "case insensitive"
    );

    sort(&mut list, Column::Bpm, true);
    assert_eq!(names(&list), vec!["alpha", "Beta", "Gamma"]);
    sort(&mut list, Column::Bpm, false);
    assert_eq!(
        names(&list),
        vec!["Beta", "alpha", "Gamma"],
        "reversing puts the fastest first and leaves the unknown last"
    );

    sort(&mut list, Column::Length, true);
    assert_eq!(names(&list), vec!["alpha", "Beta", "Gamma"]);
    sort(&mut list, Column::Key, true);
    assert_eq!(names(&list)[2], "Gamma", "no key is always last");
}

#[test]
fn search_matches_letters_in_order_and_ranks_the_closest_first() {
    let list = entries(&[
        ("Warehouse Tool.wav", None, None, None),
        ("Late Night Warehouse.wav", None, None, None),
        ("Wandering Horse.wav", None, None, None),
        ("Breakdown Edit.wav", None, None, None),
    ]);
    let found: Vec<&str> = search(&list, "ware")
        .iter()
        .map(|&i| list[i].display())
        .collect();
    assert_eq!(
        found,
        vec!["Warehouse Tool", "Late Night Warehouse", "Wandering Horse"],
        "a run of letters beats a scattered one, and the start of a name wins"
    );
    assert!(
        !found.contains(&"Breakdown Edit"),
        "a name without the letters at all is out"
    );

    let scattered: Vec<&str> = search(&list, "wh")
        .iter()
        .map(|&i| list[i].display())
        .collect();
    assert!(scattered.contains(&"Wandering Horse"), "{scattered:?}");
    assert!(search(&list, "zzz").is_empty());
    assert_eq!(search(&list, "").len(), list.len(), "no query, no filter");
    assert_eq!(
        search(&list, "WARE"),
        search(&list, "ware"),
        "case is not something to type carefully"
    );
}

#[test]
fn compatible_keys_are_the_neighbours_on_the_wheel() {
    let key = |c: &str| Key::from_camelot(c).unwrap();
    assert!(compatible(key("8A"), key("8A")), "the same key");
    assert!(compatible(key("8A"), key("9A")), "a step round the wheel");
    assert!(compatible(key("8A"), key("7A")), "and the other way");
    assert!(compatible(key("8A"), key("8B")), "the relative major");
    assert!(compatible(key("12A"), key("1A")), "the wheel wraps");
    assert!(!compatible(key("8A"), key("2A")), "across the wheel");
    assert!(
        !compatible(key("8A"), key("9B")),
        "neither number nor letter"
    );
}

#[test]
fn an_m3u_playlist_reads_as_a_list_of_tracks() {
    let dir = tempfile::tempdir().unwrap();
    let one = dir.path().join("one.wav");
    let two = dir.path().join("deep/two.flac");
    touch(&one);
    touch(&two);
    let m3u = dir.path().join("set.m3u");
    std::fs::write(
        &m3u,
        "#EXTM3U\n#EXTINF:312,Synthetic - Warehouse Tool\none.wav\ndeep/two.flac\n/gone/missing.mp3\n",
    )
    .unwrap();

    let list = Playlist::read(&m3u).unwrap();
    assert_eq!(list.name(), "set");
    assert_eq!(
        names(list.entries()),
        vec!["one", "two"],
        "missing files are dropped"
    );
}

#[test]
fn a_playlist_can_be_written_back_out() {
    let dir = tempfile::tempdir().unwrap();
    let one = dir.path().join("one.wav");
    touch(&one);
    let list = Playlist::read(&{
        let m3u = dir.path().join("set.m3u");
        std::fs::write(&m3u, "one.wav\n").unwrap();
        m3u
    })
    .unwrap();

    let out = dir.path().join("copy.m3u");
    list.write(&out).unwrap();
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.starts_with("#EXTM3U"), "{text}");
    assert!(text.contains("one.wav"), "{text}");
    assert_eq!(Playlist::read(&out).unwrap().entries().len(), 1);
}

#[test]
fn an_entry_carries_the_genre_from_its_sidecar() {
    let entry = Entry::for_test(
        Path::new("/music/a.flac"),
        "Objekt - Cactus".into(),
        Some(130.0),
        None,
        Some(400.0),
        Some("Techno, Breakbeat".into()),
    );
    assert_eq!(entry.genre(), Some("Techno, Breakbeat"));
}

#[test]
fn an_entry_with_no_sidecar_wants_both_passes() {
    // Nothing has been read, so the tag pass and the lookup both have work to do.
    let entry = Entry::for_test(
        Path::new("/music/b.flac"),
        "b".into(),
        None,
        None,
        None,
        None,
    );
    assert!(entry.needs_tags(), "its tags have never been read");
    assert!(entry.needs_lookup(), "no lookup has run over it");
}

#[test]
fn an_entry_that_already_has_a_genre_needs_no_lookup() {
    let entry = Entry::for_test(
        Path::new("/music/c.flac"),
        "c".into(),
        None,
        None,
        None,
        Some("House".into()),
    );
    assert!(!entry.needs_lookup(), "there is nothing left to find out");
    assert!(
        !entry.needs_tags(),
        "a genre can only have come from a read"
    );
}

fn track(name: &str, bpm: Option<f64>, key: Option<Key>, genre: Option<&str>) -> Entry {
    Entry::for_test(
        Path::new("/music/x.flac"),
        name.into(),
        bpm,
        key,
        Some(300.0),
        genre.map(str::to_string),
    )
}

#[test]
fn a_bpm_token_takes_the_tempo_it_rounds_to() {
    let q = Query::parse("bpm:124");
    assert!(q
        .score_entry(&track("a", Some(124.0), None, None))
        .is_some());
    assert!(q
        .score_entry(&track("b", Some(123.6), None, None))
        .is_some());
    assert!(q
        .score_entry(&track("c", Some(126.0), None, None))
        .is_none());
}

#[test]
fn a_bpm_range_is_inclusive_at_both_ends() {
    let q = Query::parse("bpm:124-128");
    for tempo in [124.0, 126.0, 128.0] {
        assert!(
            q.score_entry(&track("a", Some(tempo), None, None))
                .is_some(),
            "{tempo} is in the range"
        );
    }
    assert!(q
        .score_entry(&track("b", Some(123.0), None, None))
        .is_none());
    assert!(q
        .score_entry(&track("c", Some(129.0), None, None))
        .is_none());
}

#[test]
fn a_key_token_matches_its_camelot_code_whatever_the_case() {
    let key = Key::from_camelot("9A").expect("9A is a key");
    let other = Key::from_camelot("4A").expect("4A is a key");
    for text in ["key:9a", "key:9A"] {
        let q = Query::parse(text);
        assert!(q.score_entry(&track("a", None, Some(key), None)).is_some());
        assert!(q
            .score_entry(&track("b", None, Some(other), None))
            .is_none());
    }
}

#[test]
fn a_genre_token_is_a_substring_of_what_is_stored() {
    // Discogs joins styles, and a tag may already read like this. Asking for one of them has
    // to find it inside the joined string.
    let q = Query::parse("genre:jungle");
    assert!(q
        .score_entry(&track("a", None, None, Some("Drum n Bass, Jungle")))
        .is_some());
    assert!(q
        .score_entry(&track("b", None, None, Some("Techno")))
        .is_none());
}

#[test]
fn a_field_token_rejects_an_entry_that_has_nothing_stored() {
    assert!(Query::parse("genre:house")
        .score_entry(&track("a", None, None, None))
        .is_none());
    assert!(Query::parse("bpm:124")
        .score_entry(&track("b", None, None, None))
        .is_none());
    assert!(Query::parse("key:9a")
        .score_entry(&track("c", None, None, None))
        .is_none());
}

#[test]
fn a_token_that_will_not_parse_is_treated_as_a_word() {
    // Matching nothing would leave an empty list with no way to tell a typo from a library
    // that has nothing to show.
    assert!(Query::parse("bpm:soon")
        .score_entry(&track("bpm:soon come", None, None, None))
        .is_some());
    assert!(Query::parse("key:zz")
        .score_entry(&track("key:zz top", None, None, None))
        .is_some());
}

#[test]
fn a_query_of_only_field_tokens_keeps_every_match_on_an_equal_footing() {
    // The fuzzy needle is empty, so nothing distinguishes one match from another and the
    // order the list arrived in has to survive.
    let q = Query::parse("bpm:120-130");
    let a = q.score_entry(&track("zzz", Some(121.0), None, None));
    let b = q.score_entry(&track("aaa", Some(129.0), None, None));
    assert_eq!(
        a, b,
        "no bare words means no reason to rank one over another"
    );
    assert!(a.is_some());
}

#[test]
fn fields_and_words_narrow_together() {
    let q = Query::parse("bpm:124-128 cactus");
    assert!(q
        .score_entry(&track("Objekt - Cactus", Some(126.0), None, None))
        .is_some());
    assert!(
        q.score_entry(&track("Objekt - Cactus", Some(100.0), None, None))
            .is_none(),
        "the word matches but the tempo does not"
    );
    assert!(
        q.score_entry(&track("Bicep - Glue", Some(126.0), None, None))
            .is_none(),
        "the tempo matches but the word does not"
    );
}

#[test]
fn a_query_with_no_field_tokens_scores_as_it_always_did() {
    let entry = track("Bicep - Glue", Some(126.0), None, None);
    assert_eq!(
        Query::parse("glue").score_entry(&entry),
        library::score("glue", "Bicep - Glue")
    );
}
