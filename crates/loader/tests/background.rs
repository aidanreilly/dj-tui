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
fn a_background_run_fills_in_a_tempo_the_sidecar_never_got() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    let beat = 60.0 / 128.0;
    let mono: Vec<f32> = (0..48_000 * 12)
        .map(|i| {
            let tb = (i as f32 / 48_000.0) % beat;
            if tb < 0.02 {
                (-tb / 0.004).exp() * (std::f32::consts::TAU * 1500.0 * tb).sin()
            } else {
                0.0
            }
        })
        .collect();
    write_wav(&path, &stereo(&mono), 2, 48_000, Fmt::Float32);
    loader::load_file(&path, 48_000).unwrap();
    let sidecar = loader::sidecar::sidecar_path(&path);
    let mut j: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&sidecar).unwrap()).unwrap();
    j["analysis"]["bpm"] = serde_json::Value::Null;
    j["analysis"]["first_beat_secs"] = serde_json::Value::Null;
    std::fs::write(&sidecar, j.to_string()).unwrap();

    let loader = Loader::spawn(48_000);
    loader.analyse(path.clone());
    let start = Instant::now();
    loop {
        if loader.try_recv_analysed().is_some() {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "analysis timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let bpm = loader::sidecar::Sidecar::read(&path)
        .and_then(|s| s.analysis)
        .and_then(|a| a.grid)
        .expect("the run found a tempo")
        .bpm;
    assert!((bpm - 128.0).abs() < 0.1, "{bpm}");
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
