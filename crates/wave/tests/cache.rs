use std::fs;
use std::path::{Path, PathBuf};
use wave::{cache, WavePoint};

fn points(n: usize) -> Vec<WavePoint> {
    (0..n)
        .map(|i| {
            let v = i as f32 / n as f32;
            WavePoint {
                range: [-v, v],
                bands: [v, v * 0.5, v * 0.25],
            }
        })
        .collect()
}

/// A cache directory and a source file inside one temp dir.
fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("cache");
    fs::create_dir_all(&dir).unwrap();
    let source = tmp.path().join("track.flac");
    fs::write(&source, b"pretend audio").unwrap();
    (tmp, dir, source)
}

/// Overwrite one byte of the payload, leaving the length alone.
fn corrupt(file: &Path, at: usize) {
    let mut bytes = fs::read(file).unwrap();
    bytes[at] ^= 0xFF;
    fs::write(file, bytes).unwrap();
}

#[test]
fn a_written_cache_reads_back_identically() {
    let (_tmp, dir, source) = fixture();
    let p = points(64);
    cache::write(&dir, &source, &p);
    assert_eq!(cache::read(&dir, &source).as_deref(), Some(&p[..]));
}

#[test]
fn a_missing_cache_is_a_miss() {
    let (_tmp, dir, source) = fixture();
    assert!(cache::read(&dir, &source).is_none());
}

#[test]
fn an_empty_point_list_round_trips() {
    let (_tmp, dir, source) = fixture();
    cache::write(&dir, &source, &[]);
    assert_eq!(cache::read(&dir, &source), Some(Vec::new()));
}

#[test]
fn a_changed_modification_time_is_a_miss() {
    let (_tmp, dir, source) = fixture();
    cache::write(&dir, &source, &points(8));
    // Rewriting the source with the same length changes only its mtime.
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&source, b"pretend AUDIO").unwrap();
    assert!(cache::read(&dir, &source).is_none());
}

#[test]
fn a_changed_size_is_a_miss() {
    let (_tmp, dir, source) = fixture();
    cache::write(&dir, &source, &points(8));
    fs::write(&source, b"pretend audio, but longer").unwrap();
    assert!(cache::read(&dir, &source).is_none());
}

#[test]
fn a_truncated_file_is_a_miss() {
    let (_tmp, dir, source) = fixture();
    cache::write(&dir, &source, &points(64));
    let f = cache::path_for(&dir, &source);
    let bytes = fs::read(&f).unwrap();
    fs::write(&f, &bytes[..bytes.len() / 2]).unwrap();
    assert!(cache::read(&dir, &source).is_none());
}

#[test]
fn a_flipped_payload_byte_fails_the_checksum() {
    let (_tmp, dir, source) = fixture();
    cache::write(&dir, &source, &points(64));
    let f = cache::path_for(&dir, &source);
    let len = fs::read(&f).unwrap().len();
    corrupt(&f, len - 8);
    assert!(cache::read(&dir, &source).is_none());
}

#[test]
fn wrong_magic_is_a_miss() {
    let (_tmp, dir, source) = fixture();
    cache::write(&dir, &source, &points(8));
    corrupt(&cache::path_for(&dir, &source), 0);
    assert!(cache::read(&dir, &source).is_none());
}

#[test]
fn a_different_analysis_version_is_a_miss() {
    let (_tmp, dir, source) = fixture();
    cache::write(&dir, &source, &points(8));
    // The analysis version sits at offset 6.
    corrupt(&cache::path_for(&dir, &source), 6);
    assert!(cache::read(&dir, &source).is_none());
}

#[test]
fn a_file_written_for_another_path_is_a_miss() {
    let (_tmp, dir, source) = fixture();
    cache::write(&dir, &source, &points(8));
    // Move the cache file to where a different source would look for it.
    let other = source.parent().unwrap().join("other.flac");
    fs::copy(&source, &other).unwrap();
    fs::rename(
        cache::path_for(&dir, &source),
        cache::path_for(&dir, &other),
    )
    .unwrap();
    assert!(cache::read(&dir, &other).is_none());
}

#[test]
fn no_temporary_file_is_left_behind() {
    let (_tmp, dir, source) = fixture();
    cache::write(&dir, &source, &points(32));
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "left {leftovers:?}");
}

#[test]
fn a_missing_cache_directory_is_created() {
    let (tmp, _dir, source) = fixture();
    let fresh = tmp.path().join("not").join("there").join("yet");
    cache::write(&fresh, &source, &points(8));
    assert_eq!(cache::read(&fresh, &source).map(|p| p.len()), Some(8));
}

#[test]
fn a_write_into_an_unwritable_directory_does_not_panic() {
    let (_tmp, _dir, source) = fixture();
    // A path whose parent is a regular file can never be created.
    let blocked = source.join("cache");
    cache::write(&blocked, &source, &points(8));
    assert!(cache::read(&blocked, &source).is_none());
}

#[test]
fn a_source_path_that_is_not_utf8_is_simply_not_cached() {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let (tmp, dir, _source) = fixture();
        let name = std::ffi::OsStr::from_bytes(b"bad\xFFname.flac");
        let source = tmp.path().join(name);
        fs::write(&source, b"pretend audio").unwrap();
        cache::write(&dir, &source, &points(8));
        assert!(cache::read(&dir, &source).is_none());
    }
}

#[test]
fn two_writers_at_once_both_finish_and_the_result_is_readable() {
    let (_tmp, dir, source) = fixture();
    let p = points(256);
    std::thread::scope(|s| {
        for _ in 0..2 {
            s.spawn(|| cache::write(&dir, &source, &p));
        }
    });
    assert_eq!(cache::read(&dir, &source).as_deref(), Some(&p[..]));
}

#[test]
fn the_cache_directory_follows_xdg_then_home() {
    assert_eq!(
        cache::dir(Some("/x"), Some("/h")),
        Some(PathBuf::from("/x/dj-tui/analysis"))
    );
    assert_eq!(
        cache::dir(None, Some("/h")),
        Some(PathBuf::from("/h/.cache/dj-tui/analysis"))
    );
    assert_eq!(cache::dir(None, None), None);
}
