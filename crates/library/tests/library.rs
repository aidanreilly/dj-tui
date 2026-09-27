//! Scanning a music folder, searching it, sorting it, and spotting a compatible key.

use analysis::key::Key;
use library::{compatible, scan, search, sort, Column, Entry, Playlist};
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
