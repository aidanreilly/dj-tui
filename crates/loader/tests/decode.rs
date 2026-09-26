mod common;
use common::{sine, stereo, write_wav, Fmt};
use loader::{load_file, LoadError};

fn left(track: &engine::Track) -> Vec<f32> {
    (0..track.frames())
        .map(|i| track.frame_at(i as f64).0)
        .collect()
}

#[test]
fn decodes_stereo_pcm_at_the_session_rate_without_resampling() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.wav");
    let mut samples = Vec::new();
    for i in 0..1000 {
        samples.push(i as f32 / 1000.0);
        samples.push(-(i as f32) / 1000.0);
    }
    write_wav(&path, &samples, 2, 48_000, Fmt::Pcm16);
    let loaded = load_file(&path, 48_000, None).unwrap();
    assert_eq!(loaded.track.frames(), 1000);
    assert_eq!(loaded.track.sample_rate(), 48_000);
    let (l, r) = loaded.track.frame_at(500.0);
    assert!((l - 0.5).abs() < 1e-3 && (r + 0.5).abs() < 1e-3, "{l} {r}");
}

#[test]
fn decodes_float_wav() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f.wav");
    write_wav(&path, &stereo(&[0.25; 100]), 2, 48_000, Fmt::Float32);
    let loaded = load_file(&path, 48_000, None).unwrap();
    assert_eq!(loaded.track.frame_at(50.0), (0.25, 0.25));
}

#[test]
fn mono_is_copied_to_both_channels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.wav");
    write_wav(&path, &[0.5; 200], 1, 48_000, Fmt::Float32);
    let loaded = load_file(&path, 48_000, None).unwrap();
    assert_eq!(loaded.track.frames(), 200);
    assert_eq!(loaded.track.frame_at(100.0), (0.5, 0.5));
}

#[test]
fn resampling_keeps_duration_pitch_and_level() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.wav");
    write_wav(
        &path,
        &stereo(&sine(1000.0, 2.0, 44_100, 0.5)),
        2,
        44_100,
        Fmt::Float32,
    );
    let loaded = load_file(&path, 48_000, None).unwrap();
    assert!(
        (loaded.track.frames() as i64 - 96_000).abs() <= 2,
        "{}",
        loaded.track.frames()
    );

    // Measure over the middle second, away from edge effects.
    let l = left(&loaded.track);
    let mid = &l[24_000..72_000];
    let crossings = mid.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
    assert!(
        (crossings as i32 - 1000).abs() <= 2,
        "crossings {crossings}"
    );
    let rms = (mid.iter().map(|s| s * s).sum::<f32>() / mid.len() as f32).sqrt();
    assert!((rms - 0.5 / 2f32.sqrt()).abs() < 0.01, "rms {rms}");
}

#[test]
fn resampling_does_not_shift_timing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("i.wav");
    let mut mono = vec![0.0; 44_100 * 2];
    mono[44_100] = 1.0; // impulse at exactly one second
    write_wav(&path, &stereo(&mono), 2, 44_100, Fmt::Float32);
    let l = left(&load_file(&path, 48_000, None).unwrap().track);
    let peak = l
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .unwrap()
        .0;
    assert!((peak as i64 - 48_000).abs() <= 2, "impulse at {peak}");
}

#[test]
fn missing_file_is_an_io_error() {
    let err = load_file(std::path::Path::new("/nonexistent/x.wav"), 48_000, None).unwrap_err();
    assert!(matches!(err, LoadError::Io(_)), "{err:?}");
}

#[test]
fn garbage_is_an_unsupported_format_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("junk.mp3");
    std::fs::write(&path, vec![0x42u8; 4096]).unwrap();
    let err = load_file(&path, 48_000, None).unwrap_err();
    assert!(matches!(err, LoadError::Unsupported(_)), "{err:?}");
    assert!(!err.to_string().is_empty());
}

#[test]
fn title_falls_back_to_the_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Deep Cut (Extended).wav");
    write_wav(&path, &stereo(&[0.1; 10]), 2, 48_000, Fmt::Pcm16);
    let loaded = load_file(&path, 48_000, None).unwrap();
    assert_eq!(loaded.title, "Deep Cut (Extended)");
    assert_eq!(loaded.artist, None);
}

/// `secs` of a 440 Hz tone, stereo float WAV in its own temp dir.
fn tone_fixture(secs: f32) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("e.wav");
    let mono = sine(440.0, secs, 48_000, 0.8);
    write_wav(&path, &stereo(&mono), 2, 48_000, Fmt::Float32);
    (dir, path)
}

#[test]
fn waveform_points_are_computed_and_normalised() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("e.wav");
    let mut mono = vec![0.0; 48_000];
    mono[..24_000].iter_mut().for_each(|s| *s = 0.4);
    write_wav(&path, &stereo(&mono), 2, 48_000, Fmt::Float32);
    let loaded = load_file(&path, 48_000, None).unwrap();
    assert_eq!(loaded.waveform.len(), wave::POINTS_PER_SECOND as usize);
    assert!((loaded.waveform[0].range[1] - 1.0).abs() < 1e-6);
    let last = *loaded.waveform.last().unwrap();
    assert_eq!(last.range, [0.0, 0.0]);
    // The filters ring after the step down to silence, so allow a small tail.
    assert!(last.bands.iter().all(|b| *b < 1e-3), "{:?}", last.bands);
}

#[test]
fn point_count_follows_duration_at_twenty_per_second() {
    let (_dir, path) = tone_fixture(2.0);
    let loaded = load_file(&path, 48_000, None).unwrap();
    assert_eq!(loaded.waveform.len(), 2 * wave::POINTS_PER_SECOND as usize);
}

#[test]
fn the_envelope_does_not_depend_on_the_session_rate() {
    // Analysis runs before resampling, so both sessions see the same picture.
    let (_dir, path) = tone_fixture(1.0);
    let a = load_file(&path, 44_100, None).unwrap().waveform;
    let b = load_file(&path, 48_000, None).unwrap().waveform;
    assert_eq!(a, b);
}

#[test]
fn the_second_load_comes_from_the_cache() {
    let cache = tempfile::tempdir().unwrap();
    let (_dir, path) = tone_fixture(1.0);
    let first = load_file(&path, 48_000, Some(cache.path()))
        .unwrap()
        .waveform;
    assert!(wave::cache::path_for(cache.path(), &path).exists());
    let second = load_file(&path, 48_000, Some(cache.path()))
        .unwrap()
        .waveform;
    assert_eq!(first, second);
}

#[test]
fn a_cache_file_that_is_not_one_is_ignored() {
    let cache = tempfile::tempdir().unwrap();
    let (_dir, path) = tone_fixture(1.0);
    // The right name with nonsense inside must not be trusted.
    std::fs::write(wave::cache::path_for(cache.path(), &path), b"not a cache").unwrap();
    let loaded = load_file(&path, 48_000, Some(cache.path())).unwrap();
    assert_eq!(loaded.waveform.len(), wave::POINTS_PER_SECOND as usize);
}
