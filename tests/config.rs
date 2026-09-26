use dj_tui::config::{config_path, Backend, Config, WaveformMode};
use engine::CrossfaderCurve;
use std::path::PathBuf;

#[test]
fn empty_file_gives_defaults() {
    let c = Config::from_toml("").unwrap();
    assert_eq!(c, Config::default());
    assert_eq!(c.audio.backend, Backend::Jack);
    assert_eq!(c.audio.sample_rate, 48_000);
    assert_eq!(c.audio.buffer_frames, 256);
    assert_eq!(c.ui.waveform_mode, WaveformMode::ThreeBand);
    assert_eq!(c.ui.end_warning_secs, 30);
    assert_eq!(c.deck.tempo_range, 8);
    assert_eq!(c.mixer.crossfader_curve, CrossfaderCurve::ConstantPower);
}

#[test]
fn partial_file_overrides_only_what_it_sets() {
    let c = Config::from_toml(
        r#"
        [audio]
        backend = "alsa"
        buffer_frames = 128
        [ui]
        waveform_mode = "rgb"
        [mixer]
        crossfader_curve = "cut"
        "#,
    )
    .unwrap();
    assert_eq!(c.audio.backend, Backend::Alsa);
    assert_eq!(c.audio.buffer_frames, 128);
    assert_eq!(c.audio.sample_rate, 48_000);
    assert_eq!(c.ui.waveform_mode, WaveformMode::Rgb);
    assert_eq!(c.mixer.crossfader_curve, CrossfaderCurve::Cut);
}

#[test]
fn unknown_values_are_rejected() {
    assert!(Config::from_toml("[audio]\nbackend = \"pulse\"").is_err());
    assert!(Config::from_toml("[audio]\nbogus = 1").is_err());
}

#[test]
fn tempo_range_must_be_a_supported_value() {
    assert!(Config::from_toml("[deck]\ntempo_range = 16").is_ok());
    let err = Config::from_toml("[deck]\ntempo_range = 12").unwrap_err();
    assert!(err.to_string().contains("tempo_range"), "{err}");
}

#[test]
fn buffer_size_must_be_a_power_of_two() {
    assert!(Config::from_toml("[audio]\nbuffer_frames = 100").is_err());
}

#[test]
fn config_path_prefers_xdg_then_home() {
    assert_eq!(
        config_path(Some("/x"), Some("/home/u")),
        Some(PathBuf::from("/x/dj-tui/config.toml"))
    );
    assert_eq!(
        config_path(None, Some("/home/u")),
        Some(PathBuf::from("/home/u/.config/dj-tui/config.toml"))
    );
    assert_eq!(config_path(Some(""), Some("/home/u")), config_path(None, Some("/home/u")));
    assert_eq!(config_path(None, None), None);
}

#[test]
fn waveform_mode_cycles_through_all_three() {
    let m = WaveformMode::ThreeBand;
    assert_eq!(m.next(), WaveformMode::Rgb);
    assert_eq!(m.next().next(), WaveformMode::Blue);
    assert_eq!(m.next().next().next(), WaveformMode::ThreeBand);
}
