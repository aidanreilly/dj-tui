mod common;
use common::{stereo, write_wav, Fmt};
use engine::DeckId;
use loader::Loader;
use std::time::{Duration, Instant};

fn wait(loader: &Loader) -> loader::LoadResult {
    let start = Instant::now();
    loop {
        if let Some(r) = loader.try_recv() {
            return r;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "loader timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn loads_happen_off_thread_and_report_the_deck() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bg.wav");
    write_wav(&path, &stereo(&[0.2; 480]), 2, 48_000, Fmt::Pcm16);
    let loader = Loader::spawn(48_000);
    assert!(loader.try_recv().is_none());
    loader.request(DeckId::B, path.clone());
    let r = wait(&loader);
    assert_eq!(r.deck, DeckId::B);
    assert_eq!(r.path, path);
    assert_eq!(r.result.unwrap().track.frames(), 480);
}

#[test]
fn failures_are_reported_not_panicked() {
    let loader = Loader::spawn(48_000);
    loader.request(DeckId::A, "/nope.flac".into());
    assert!(wait(&loader).result.is_err());
}

#[test]
fn analysing_in_the_background_writes_the_sidecar_and_says_when_it_is_done() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("analyse me.wav");
    write_wav(&path, &stereo(&[0.3; 48_000]), 2, 48_000, Fmt::Pcm16);
    let loader = Loader::spawn(48_000);
    loader.analyse(path.clone());

    let start = Instant::now();
    let done = loop {
        if let Some(done) = loader.try_recv_analysed() {
            break done;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "analysis timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(done.path, path);
    assert!(done.error.is_none(), "{:?}", done.error);
    assert!(
        loader::sidecar::Sidecar::read(&path)
            .and_then(|s| s.analysis)
            .is_some(),
        "the analysis is saved beside the file"
    );
}

#[test]
fn a_file_that_cannot_be_analysed_comes_back_with_the_reason() {
    let loader = Loader::spawn(48_000);
    loader.analyse("/definitely/missing.flac".into());
    let start = Instant::now();
    let done = loop {
        if let Some(done) = loader.try_recv_analysed() {
            break done;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "timed out");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(done.error.is_some());
}
