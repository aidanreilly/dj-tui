//! Controllers: loading mappings from disk and keeping one in step with the app.
//!
//! The device end lives in `backend`, which owns the JACK client and its MIDI ports. This
//! module is what sits between those bytes and the app.

use crate::app::App;
use crate::config::Midi as MidiConfig;
use engine::DeckId;
use midi::{Control, Feedback, LedState, Mapping, Message, Outcome};
use std::path::{Path, PathBuf};

/// Every control a mapping might hold a position for, so soft takeover can be kept in step
/// without the app knowing what a mapping contains.
const TRACKED: &[Control] = &[
    Control::Crossfader,
    Control::CueMix,
    Control::Fader(DeckId::A),
    Control::Fader(DeckId::B),
    Control::Tempo(DeckId::A),
    Control::Tempo(DeckId::B),
    Control::Trim(DeckId::A),
    Control::Trim(DeckId::B),
    Control::Filter,
    Control::Eq(DeckId::A, input::Band::Low),
    Control::Eq(DeckId::A, input::Band::Mid),
    Control::Eq(DeckId::A, input::Band::High),
    Control::Eq(DeckId::B, input::Band::Low),
    Control::Eq(DeckId::B, input::Band::Mid),
    Control::Eq(DeckId::B, input::Band::High),
];

/// A loaded mapping and the lights that go with it.
pub struct Controller {
    mapping: Mapping,
    feedback: Feedback,
}

impl Controller {
    pub fn new(mapping: Mapping, soft_takeover: bool) -> Self {
        let mut mapping = mapping;
        if !soft_takeover {
            mapping.engage_all();
        }
        let feedback = Feedback::new(&mapping);
        Self { mapping, feedback }
    }

    pub fn name(&self) -> &str {
        self.mapping.name()
    }

    /// Port names this mapping wants connected, for the backend's hotplug scan.
    pub fn wanted_ports(&self) -> Vec<String> {
        self.mapping.ports().to_vec()
    }

    /// Feed one message in. Returns true when the user asked to quit.
    pub fn handle(&mut self, app: &mut App, bytes: [u8; 3]) -> bool {
        let Some(message) = Message::from_bytes(&bytes) else {
            return false;
        };
        match self.mapping.handle(message) {
            Outcome::Nothing => false,
            Outcome::Act(action) => app.on_action(action),
            Outcome::Set(control, value) => {
                app.set_control(control, value);
                false
            }
        }
    }

    /// Tell the mapping where the app has each control, so a knob that has not caught up
    /// stays quiet. Cheap: the mapping only reacts to the controls it actually holds.
    pub fn sync(&mut self, app: &App) {
        for control in TRACKED {
            self.mapping
                .sync_control(*control, app.control_value(*control));
        }
    }

    /// Messages for lights that have changed since the last call.
    pub fn lights(&mut self, app: &App) -> Vec<Message> {
        let snapshot = app.snapshot();
        self.feedback.update(|state| match state {
            LedState::Playing(d) => snapshot.decks[d.index()].playing,
            LedState::Cued(d) => {
                let deck = &snapshot.decks[d.index()];
                !deck.playing && deck.track_frames > 0 && deck.position == deck.cue_point
            }
            LedState::Loop(d) => snapshot.decks[d.index()].loop_span.is_some(),
            LedState::HotCue(d, n) => snapshot.decks[d.index()]
                .hot_cues
                .get(n)
                .is_some_and(|c| c.is_some()),
            LedState::KeyLock(d) => app.key_lock(d),
            LedState::Quantize(d) => app.quantize(d),
            // Sync is a press, not a state the deck holds; the light follows the tempo
            // matching the other deck, which is what the button did.
            LedState::Sync(d) => app.tempo_matches_other_deck(d),
        })
    }
}

/// Load every mapping the config asks for. Returns what loaded and what did not, so the
/// caller can say so in the message line rather than failing to start.
pub fn load_mappings(config: &MidiConfig, dir: Option<&Path>) -> (Vec<Controller>, Vec<String>) {
    let mut loaded = Vec::new();
    let mut problems = Vec::new();
    if !config.enabled {
        return (loaded, problems);
    }
    let Some(dir) = dir else {
        return (loaded, problems);
    };
    for path in mapping_files(config, dir) {
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()) {
            Ok(text) => match Mapping::from_toml(&text) {
                Ok(mapping) => loaded.push(Controller::new(mapping, config.soft_takeover)),
                Err(e) => problems.push(format!("{}: {e}", path.display())),
            },
            Err(e) => problems.push(format!("{}: {e}", path.display())),
        }
    }
    (loaded, problems)
}

/// The files to read: those listed in the config, or every `.toml` in the mappings directory.
fn mapping_files(config: &MidiConfig, dir: &Path) -> Vec<PathBuf> {
    if !config.mappings.is_empty() {
        return config
            .mappings
            .iter()
            .map(|name| {
                let path = PathBuf::from(name);
                if path.is_absolute() {
                    path
                } else {
                    dir.join(path)
                }
            })
            .collect();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    found.sort();
    found
}
