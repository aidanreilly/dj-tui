//! Track data saved beside the audio file: `Song.flac` -> `Song.flac.dj-tui.json`.
//! Holds analysis (BPM, beat grid, key), the overview waveform and cues, so a second load
//! skips analysis and brings the cues back.

mod common;
use common::{stereo, write_wav, Fmt};
use loader::load_file;
use loader::sidecar::{fingerprint_file, save_cues, sidecar_path, Cues};
use serde_json::Value;
use std::path::{Path, PathBuf};

const RATE: u32 = 48_000;

fn clicks(path: &Path, bpm: f32, secs: f32) {
    let beat = 60.0 / bpm;
    let mono: Vec<f32> = (0..(RATE as f32 * secs) as usize)
        .map(|i| {
            let tb = (i as f32 / RATE as f32) % beat;
            if tb < 0.02 {
                (-tb / 0.004).exp() * (std::f32::consts::TAU * 1500.0 * tb).sin()
            } else {
                0.0
            }
        })
        .collect();
    write_wav(path, &stereo(&mono), 2, RATE, Fmt::Float32);
}

fn track(dir: &tempfile::TempDir) -> PathBuf {
    let p = dir.path().join("Warehouse Tool.wav");
    clicks(&p, 128.0, 12.0);
    p
}

fn json(audio: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(sidecar_path(audio)).unwrap()).unwrap()
}

#[test]
fn sidecar_sits_beside_the_audio_file() {
    assert_eq!(
        sidecar_path(Path::new("/music/Song.mp3")),
        PathBuf::from("/music/Song.mp3.dj-tui.json")
    );
}

#[test]
fn fingerprint_is_fnv1a_of_the_file_start() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));
    std::fs::write(&a, b"").unwrap();
    std::fs::write(&b, b"a").unwrap();
    assert_eq!(
        fingerprint_file(&a).unwrap().fingerprint,
        0xcbf2_9ce4_8422_2325
    );
    assert_eq!(
        fingerprint_file(&b).unwrap().fingerprint,
        0xaf63_dc4c_8601_ec8c
    );
    assert_eq!(fingerprint_file(&b).unwrap().file_size, 1);
}

#[test]
fn first_load_writes_analysis_waveform_and_cues() {
    let dir = tempfile::tempdir().unwrap();
    let audio = track(&dir);
    let loaded = load_file(&audio, RATE).unwrap();
    assert!(!loaded.from_sidecar);
    let j = json(&audio);
    assert_eq!(j["version"], 1);
    assert!((j["analysis"]["bpm"].as_f64().unwrap() - 128.0).abs() < 0.05);
    assert!(j["analysis"]["first_beat_secs"].is_number());
    assert!(j["analysis"]["key"].is_string() || j["analysis"]["key"].is_null());
    assert_eq!(j["waveform"]["points"], loader::ENVELOPE_POINTS);
    assert_eq!(
        j["waveform"]["ranges"].as_array().unwrap().len(),
        loader::ENVELOPE_POINTS
    );
    assert_eq!(
        j["waveform"]["bands"].as_array().unwrap().len(),
        loader::ENVELOPE_POINTS
    );
    assert!(j["cues"]["hot_cues"].as_array().unwrap().is_empty());
}

#[test]
fn second_load_reuses_the_saved_analysis() {
    let dir = tempfile::tempdir().unwrap();
    let audio = track(&dir);
    load_file(&audio, RATE).unwrap();
    // Edit the saved BPM, as a user fixing a wrong detection would.
    let mut j = json(&audio);
    j["analysis"]["bpm"] = 99.5.into();
    std::fs::write(sidecar_path(&audio), j.to_string()).unwrap();
    let again = load_file(&audio, RATE).unwrap();
    assert!(again.from_sidecar);
    assert_eq!(again.grid.unwrap().bpm, 99.5);
}

#[test]
fn saved_waveform_round_trips_closely() {
    let dir = tempfile::tempdir().unwrap();
    let audio = track(&dir);
    let first = load_file(&audio, RATE).unwrap();
    let again = load_file(&audio, RATE).unwrap();
    for (a, b) in first.waveform.iter().zip(&again.waveform) {
        assert!((a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01);
    }
    for (a, b) in first.bands.iter().zip(&again.bands) {
        assert!((0..3).all(|i| (a[i] - b[i]).abs() < 0.01));
    }
    assert_eq!(first.key, again.key);
}

#[test]
fn changed_audio_is_analysed_again() {
    let dir = tempfile::tempdir().unwrap();
    let audio = track(&dir);
    load_file(&audio, RATE).unwrap();
    clicks(&audio, 140.0, 12.0);
    let again = load_file(&audio, RATE).unwrap();
    assert!(!again.from_sidecar);
    let bpm = again.grid.unwrap().bpm;
    // This test is about re-analysis; a 12 s clip gives a little less precision than a track.
    assert!((bpm - 140.0).abs() < 0.1, "{bpm}");
    assert!((json(&audio)["analysis"]["bpm"].as_f64().unwrap() - 140.0).abs() < 0.1);
}

#[test]
fn a_corrupt_sidecar_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let audio = track(&dir);
    std::fs::write(sidecar_path(&audio), "{ not json").unwrap();
    let loaded = load_file(&audio, RATE).unwrap();
    assert!(!loaded.from_sidecar);
    assert_eq!(json(&audio)["version"], 1);
}

#[test]
fn cues_are_saved_and_restored_without_touching_analysis() {
    let dir = tempfile::tempdir().unwrap();
    let audio = track(&dir);
    load_file(&audio, RATE).unwrap();
    let mut cues = Cues {
        main_cue_secs: Some(1.5),
        ..Default::default()
    };
    cues.hot_cues[0] = Some(3.0);
    cues.hot_cues[6] = Some(9.25);
    save_cues(&audio, &cues).unwrap();
    let j = json(&audio);
    assert_eq!(j["cues"]["hot_cues"][1]["pad"], 7);
    let again = load_file(&audio, RATE).unwrap();
    assert!(again.from_sidecar, "saving cues kept the analysis valid");
    assert_eq!(again.cues, cues);
}

#[test]
fn cues_saved_before_the_first_analysis_survive_it() {
    let dir = tempfile::tempdir().unwrap();
    let audio = track(&dir);
    let cues = Cues {
        main_cue_secs: Some(2.0),
        ..Default::default()
    };
    save_cues(&audio, &cues).unwrap();
    let loaded = load_file(&audio, RATE).unwrap();
    assert_eq!(loaded.cues, cues);
    assert!((json(&audio)["analysis"]["bpm"].as_f64().unwrap() - 128.0).abs() < 0.05);
}

#[test]
fn an_unwritable_sidecar_does_not_stop_loading() {
    let dir = tempfile::tempdir().unwrap();
    let audio = track(&dir);
    // A directory where the file should go blocks the write, even for root.
    std::fs::create_dir(sidecar_path(&audio)).unwrap();
    let loaded = load_file(&audio, RATE).unwrap();
    assert!(
        loaded
            .sidecar_note
            .as_deref()
            .is_some_and(|n| n.contains("dj-tui.json")),
        "{:?}",
        loaded.sidecar_note
    );
    assert!(loaded.grid.is_some());
}
