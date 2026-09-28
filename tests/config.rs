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
    assert_eq!(
        config_path(Some(""), Some("/home/u")),
        config_path(None, Some("/home/u"))
    );
    assert_eq!(config_path(None, None), None);
}

#[test]
fn waveform_mode_cycles_through_all_three() {
    let m = WaveformMode::ThreeBand;
    assert_eq!(m.next(), WaveformMode::Rgb);
    assert_eq!(m.next().next(), WaveformMode::Blue);
    assert_eq!(m.next().next().next(), WaveformMode::ThreeBand);
}

#[test]
fn routing_defaults_to_auto() {
    assert_eq!(Config::default().routing().unwrap(), backend::Routing::Auto);
}

#[test]
fn routing_can_be_turned_off() {
    let c = Config::from_toml("[audio]\nrouting = \"off\"").unwrap();
    assert_eq!(c.routing().unwrap(), backend::Routing::Off);
}

#[test]
fn explicit_routing_reads_port_lists() {
    let c = Config::from_toml(
        "[audio]\nrouting = \"explicit\"\nmaster_ports = [\"a:1\", \"a:2\"]\ncue_ports = [\"b:1\", \"b:2\"]",
    )
    .unwrap();
    assert_eq!(
        c.routing().unwrap(),
        backend::Routing::Explicit {
            master: ["a:1".into(), "a:2".into()],
            cue: Some(["b:1".into(), "b:2".into()]),
        }
    );
}

#[test]
fn explicit_routing_needs_exactly_two_master_ports() {
    assert!(Config::from_toml("[audio]\nrouting = \"explicit\"").is_err());
    assert!(
        Config::from_toml("[audio]\nrouting = \"explicit\"\nmaster_ports = [\"a:1\"]").is_err()
    );
    assert!(Config::from_toml(
        "[audio]\nrouting = \"explicit\"\nmaster_ports = [\"a:1\",\"a:2\"]\ncue_ports = [\"x\"]"
    )
    .is_err());
}

#[test]
fn client_name_defaults_to_dj_tui() {
    assert_eq!(Config::default().audio.client_name, "dj-tui");
}

#[test]
fn graphics_defaults_to_auto_and_can_be_turned_off() {
    use dj_tui::config::Graphics;
    assert_eq!(Config::default().ui.graphics, Graphics::Auto);
    assert_eq!(
        Config::from_toml("[ui]\ngraphics = \"off\"")
            .unwrap()
            .ui
            .graphics,
        Graphics::Off
    );
    assert!(Config::from_toml("[ui]\ngraphics = \"sometimes\"").is_err());
}

#[test]
fn split_routing_mode() {
    let c = Config::from_toml("[audio]\nrouting = \"split\"").unwrap();
    assert_eq!(c.routing().unwrap(), backend::Routing::Split);
}

#[test]
fn midi_defaults_to_on_with_soft_takeover() {
    let c = Config::default();
    assert!(c.midi.enabled);
    assert!(c.midi.soft_takeover);
    assert!(c.midi.mappings.is_empty(), "every mapping found is loaded");

    let c = Config::from_toml("[midi]\nenabled = false\nmappings = [\"ddj.toml\"]").unwrap();
    assert!(!c.midi.enabled);
    assert_eq!(c.midi.mappings, vec!["ddj.toml".to_string()]);
    assert!(
        Config::from_toml("[midi]\nwobble = 1").is_err(),
        "typos are caught"
    );
}

#[test]
fn library_folders_expand_a_leading_tilde() {
    use std::path::PathBuf;
    let c = Config::from_toml("[library]\nfolders = [\"~/Music\", \"/mnt/dj\"]").unwrap();
    assert_eq!(
        c.library.paths(Some("/home/dj")),
        vec![PathBuf::from("/home/dj/Music"), PathBuf::from("/mnt/dj")]
    );
    assert!(Config::default().library.folders.is_empty());
}

#[test]
fn saving_a_device_leaves_the_rest_of_the_file_alone() {
    use dj_tui::config::with_device;

    let original = "# my settings\n[audio]\nclient_name = \"dj-tui\"\ndevice = \"default\"\n\n[ui]\ngraphics = \"off\"\n";
    let updated = with_device(original, "hw:1,0");
    assert!(updated.contains("# my settings"), "{updated}");
    assert!(updated.contains("device = \"hw:1,0\""), "{updated}");
    assert!(!updated.contains("\"default\""), "{updated}");
    assert!(updated.contains("graphics = \"off\""), "{updated}");
    assert_eq!(
        Config::from_toml(&updated).unwrap().audio.device,
        "hw:1,0",
        "and it still parses"
    );
}

#[test]
fn saving_a_device_adds_what_the_file_is_missing() {
    use dj_tui::config::with_device;

    let added = with_device("[ui]\ngraphics = \"off\"\n", "hw:0,0");
    assert!(added.contains("[audio]"), "{added}");
    assert_eq!(Config::from_toml(&added).unwrap().audio.device, "hw:0,0");

    let section_only = with_device("[audio]\nclient_name = \"x\"\n", "pipewire");
    assert_eq!(
        Config::from_toml(&section_only).unwrap().audio.device,
        "pipewire"
    );

    let empty = with_device("", "default");
    assert_eq!(Config::from_toml(&empty).unwrap().audio.device, "default");
}

#[test]
fn a_fade_length_outside_the_range_is_rejected_by_name() {
    let err = Config::from_toml("[mixer]\nfade_beats = 0\n").unwrap_err();
    assert!(err.contains("fade_beats"), "got {err}");
    assert!(
        err.contains("2") && err.contains("64"),
        "names the range: {err}"
    );
    assert!(Config::from_toml("[mixer]\nfade_beats = 500\n").is_err());
    assert_eq!(
        Config::from_toml("[mixer]\nfade_beats = 16\n")
            .unwrap()
            .mixer
            .fade_beats,
        16.0
    );
}

#[test]
fn the_fade_length_defaults_to_eight_beats() {
    assert_eq!(Config::default().mixer.fade_beats, 8.0);
}
