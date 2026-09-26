//! `~/.config/dj-tui/config.toml`. Every field is optional; unknown keys are an error.

use engine::CrossfaderCurve;
use serde::{Deserialize, Deserializer};
use std::path::PathBuf;

const TEMPO_RANGES: [u8; 3] = [8, 16, 50];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Jack,
    Alsa,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum WaveformMode {
    #[default]
    #[serde(rename = "3band")]
    ThreeBand,
    #[serde(rename = "rgb")]
    Rgb,
    #[serde(rename = "blue")]
    Blue,
}

impl WaveformMode {
    pub fn next(self) -> Self {
        match self {
            Self::ThreeBand => Self::Rgb,
            Self::Rgb => Self::Blue,
            Self::Blue => Self::ThreeBand,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub audio: Audio,
    pub ui: Ui,
    pub deck: Deck,
    pub mixer: Mixer,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Audio {
    pub backend: Backend,
    pub sample_rate: u32,
    pub buffer_frames: u32,
}

impl Default for Audio {
    fn default() -> Self {
        Self { backend: Backend::Jack, sample_rate: 48_000, buffer_frames: 256 }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Ui {
    pub waveform_mode: WaveformMode,
    pub end_warning_secs: u32,
}

impl Default for Ui {
    fn default() -> Self {
        Self { waveform_mode: WaveformMode::ThreeBand, end_warning_secs: 30 }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Deck {
    /// Tempo fader range in percent: 8, 16 or 50.
    pub tempo_range: u8,
}

impl Default for Deck {
    fn default() -> Self {
        Self { tempo_range: 8 }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Mixer {
    #[serde(deserialize_with = "curve")]
    pub crossfader_curve: CrossfaderCurve,
}

fn curve<'de, D: Deserializer<'de>>(d: D) -> Result<CrossfaderCurve, D::Error> {
    let s = String::deserialize(d)?;
    match s.as_str() {
        "linear" => Ok(CrossfaderCurve::Linear),
        "constant-power" => Ok(CrossfaderCurve::ConstantPower),
        "cut" => Ok(CrossfaderCurve::Cut),
        other => Err(serde::de::Error::unknown_variant(other, &["linear", "constant-power", "cut"])),
    }
}

impl Config {
    pub fn from_toml(s: &str) -> Result<Self, String> {
        let c: Config = toml::from_str(s).map_err(|e| e.to_string())?;
        c.validate()?;
        Ok(c)
    }

    fn validate(&self) -> Result<(), String> {
        if !TEMPO_RANGES.contains(&self.deck.tempo_range) {
            return Err(format!(
                "deck.tempo_range must be one of {TEMPO_RANGES:?}, got {}",
                self.deck.tempo_range
            ));
        }
        let b = self.audio.buffer_frames;
        let max = engine::MAX_BLOCK_FRAMES as u32;
        if !b.is_power_of_two() || !(16..=max).contains(&b) {
            return Err(format!("audio.buffer_frames must be a power of two from 16 to {max}, got {b}"));
        }
        Ok(())
    }
}

/// `$XDG_CONFIG_HOME/dj-tui/config.toml`, falling back to `~/.config/dj-tui/config.toml`.
pub fn config_path(xdg_config_home: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    let base = match xdg_config_home.filter(|s| !s.is_empty()) {
        Some(x) => PathBuf::from(x),
        None => PathBuf::from(home.filter(|s| !s.is_empty())?).join(".config"),
    };
    Some(base.join("dj-tui").join("config.toml"))
}
