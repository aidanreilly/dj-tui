//! Controller mappings: MIDI messages in, the same `Action`s the keyboard produces out, plus
//! absolute moves for faders and knobs that a stepped action cannot express.
//!
//! A mapping is a TOML file naming what each control does. Nothing here talks to a device;
//! the ports live in the binary, which keeps this whole layer testable from a string.

mod message;

pub use message::{Address, Message};

use engine::DeckId;
use input::{parse_action, Action, Band};
use serde::Deserialize;
use std::collections::HashMap;

/// How close a knob has to come to the value on screen before it takes over, as a fraction.
const TAKEOVER_TOLERANCE: f32 = 0.02;

/// A control the UI holds an absolute value for. Stepping these from MIDI would throw away
/// where the hardware is pointing, so they are set outright instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    Crossfader,
    Fader(DeckId),
    Tempo(DeckId),
    Trim(DeckId),
    Eq(DeckId, Band),
    Filter(DeckId),
    FxWet(DeckId),
    FxParam(DeckId, usize),
    CueMix,
}

impl Control {
    /// Parse a control name: `fader a`, `eq b mid`, `fx param a 2`, `crossfader`.
    pub fn parse(text: &str) -> Option<Control> {
        let lower = text.to_ascii_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        let deck = |w: &str| match w {
            "a" => Some(DeckId::A),
            "b" => Some(DeckId::B),
            _ => None,
        };
        let band = |w: &str| match w {
            "high" => Some(Band::High),
            "mid" => Some(Band::Mid),
            "low" => Some(Band::Low),
            _ => None,
        };
        Some(match words.as_slice() {
            ["crossfader"] => Control::Crossfader,
            ["cue-mix"] => Control::CueMix,
            ["fader", d] => Control::Fader(deck(d)?),
            ["tempo", d] => Control::Tempo(deck(d)?),
            ["trim", d] => Control::Trim(deck(d)?),
            ["filter", d] => Control::Filter(deck(d)?),
            ["eq", d, b] => Control::Eq(deck(d)?, band(b)?),
            ["fx", "wet", d] => Control::FxWet(deck(d)?),
            ["fx", "param", d, knob] => {
                let n: usize = knob.parse().ok()?;
                Control::FxParam(deck(d)?, (1..=2).contains(&n).then(|| n - 1)?)
            }
            _ => return None,
        })
    }
}

/// What one message came to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    Nothing,
    Act(Action),
    /// Move a control to a value from 0 to 1.
    Set(Control, f32),
}

#[derive(Debug)]
struct Button {
    press: Action,
    /// What to send on release, for controls that need the key held.
    release: Option<Action>,
}

#[derive(Debug)]
struct Knob {
    control: Control,
    /// False until the hardware has caught up with the value on screen.
    engaged: bool,
    /// Where the UI has the control, which is what the hardware has to reach.
    ui: f32,
    /// The last position the hardware reported, for spotting a crossing.
    last: Option<f32>,
}

#[derive(Debug)]
struct Encoder {
    up: Action,
    down: Action,
}

#[derive(Debug)]
struct Led {
    address: Address,
    state: String,
}

/// One controller's mapping.
#[derive(Debug)]
pub struct Mapping {
    name: String,
    ports: Vec<String>,
    buttons: HashMap<Address, Button>,
    knobs: HashMap<Address, Knob>,
    encoders: HashMap<Address, Encoder>,
    leds: Vec<Led>,
}

#[derive(Deserialize)]
struct RawMapping {
    name: String,
    #[serde(default)]
    ports: Vec<String>,
    #[serde(default)]
    buttons: Vec<RawButton>,
    #[serde(default)]
    knobs: Vec<RawKnob>,
    #[serde(default)]
    encoders: Vec<RawEncoder>,
    #[serde(default)]
    leds: Vec<RawLed>,
}

#[derive(Deserialize)]
struct RawButton {
    input: String,
    action: String,
    /// True for controls that need the button held, such as cue.
    #[serde(default)]
    hold: bool,
}

#[derive(Deserialize)]
struct RawKnob {
    input: String,
    control: String,
}

#[derive(Deserialize)]
struct RawEncoder {
    input: String,
    action: String,
}

#[derive(Deserialize)]
struct RawLed {
    output: String,
    state: String,
}

impl Mapping {
    pub fn from_toml(text: &str) -> Result<Mapping, String> {
        let raw: RawMapping = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut mapping = Mapping {
            name: raw.name,
            ports: raw.ports,
            buttons: HashMap::new(),
            knobs: HashMap::new(),
            encoders: HashMap::new(),
            leds: Vec::new(),
        };
        for b in raw.buttons {
            let address = Address::parse(&b.input)?;
            let press = action(&b.action)?;
            let release =
                if b.hold {
                    Some(release_of(press).ok_or_else(|| {
                        format!("{:?} is not a button that can be held", b.action)
                    })?)
                } else {
                    None
                };
            mapping.buttons.insert(address, Button { press, release });
        }
        for k in raw.knobs {
            let address = Address::parse(&k.input)?;
            let control = Control::parse(&k.control)
                .ok_or_else(|| format!("{:?} is not a control this can set", k.control))?;
            mapping.knobs.insert(
                address,
                Knob {
                    control,
                    engaged: false,
                    ui: 0.0,
                    last: None,
                },
            );
        }
        for e in raw.encoders {
            let address = Address::parse(&e.input)?;
            // An encoder's action needs a direction, which the file leaves off.
            let up = action(&format!("{} up", e.action))?;
            let down = action(&format!("{} down", e.action))?;
            mapping.encoders.insert(address, Encoder { up, down });
        }
        for l in raw.leds {
            mapping.leds.push(Led {
                address: Address::parse(&l.output)?,
                state: l.state,
            });
        }
        Ok(mapping)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// True when this mapping is for the named port.
    pub fn matches_port(&self, port: &str) -> bool {
        let port = port.to_ascii_lowercase();
        self.ports
            .iter()
            .any(|want| port.contains(&want.to_ascii_lowercase()))
    }

    /// The control a mapping file address is bound to, for tests and for `dj-tui --midi-list`.
    pub fn control_for(&self, address: &str) -> Option<Control> {
        let address = Address::parse(address).ok()?;
        self.knobs.get(&address).map(|k| k.control)
    }

    /// Tell the mapping where the UI has a control. A knob has to reach this before it takes
    /// over, so a fader left at the wrong end cannot slam the sound when it first moves.
    pub fn sync_control(&mut self, control: Control, value: f32) {
        for knob in self.knobs.values_mut() {
            if knob.control == control && (knob.ui - value).abs() > f32::EPSILON {
                knob.ui = value.clamp(0.0, 1.0);
                knob.engaged = false;
                knob.last = None;
            }
        }
    }

    /// Drop soft takeover, for a controller that is already where the screen is.
    pub fn engage_all(&mut self) {
        for knob in self.knobs.values_mut() {
            knob.engaged = true;
        }
    }

    /// Turn one message into what it does.
    pub fn handle(&mut self, message: Message) -> Outcome {
        let address = Address::of(message);
        match message {
            Message::NoteOn { velocity: 0, .. } | Message::NoteOff { .. } => self
                .buttons
                .get(&address)
                .and_then(|b| b.release)
                .map_or(Outcome::Nothing, Outcome::Act),
            Message::NoteOn { .. } => self
                .buttons
                .get(&address)
                .map_or(Outcome::Nothing, |b| Outcome::Act(b.press)),
            Message::Cc { value, .. } => {
                if let Some(encoder) = self.encoders.get(&address) {
                    // Two's complement around 64: 1..63 turns up, 65..127 turns down.
                    return match value {
                        1..=63 => Outcome::Act(encoder.up),
                        65..=127 => Outcome::Act(encoder.down),
                        _ => Outcome::Nothing,
                    };
                }
                self.knob(address, value as f32 / 127.0)
            }
            Message::PitchBend { value, .. } => self.knob(address, value as f32 / 16_383.0),
        }
    }

    /// A knob move, held back until it has caught up with the value on screen.
    fn knob(&mut self, address: Address, value: f32) -> Outcome {
        let Some(knob) = self.knobs.get_mut(&address) else {
            return Outcome::Nothing;
        };
        if !knob.engaged {
            let close = (value - knob.ui).abs() <= TAKEOVER_TOLERANCE;
            let crossed = knob
                .last
                .is_some_and(|last| (last - knob.ui).signum() != (value - knob.ui).signum());
            knob.last = Some(value);
            if !close && !crossed {
                return Outcome::Nothing;
            }
            knob.engaged = true;
        }
        knob.ui = value;
        Outcome::Set(knob.control, value)
    }

    /// Addresses this mapping lights up, with the state each one follows.
    pub fn leds(&self) -> impl Iterator<Item = (Address, &str)> {
        self.leds.iter().map(|l| (l.address, l.state.as_str()))
    }
}

fn action(text: &str) -> Result<Action, String> {
    parse_action(text).ok_or_else(|| format!("{text:?} is not an action"))
}

/// The release half of a button that is held.
fn release_of(press: Action) -> Option<Action> {
    match press {
        Action::CuePress(d) => Some(Action::CueRelease(d)),
        _ => None,
    }
}

/// The mapping file lines for a message learned from the hardware.
pub fn learn_line(message: Message, target: &str) -> String {
    let address = message.address();
    if parse_action(target).is_some() {
        let section = match message {
            Message::NoteOn { .. } | Message::NoteOff { .. } => "buttons",
            _ => "encoders",
        };
        format!("[[{section}]]\ninput = \"{address}\"\naction = \"{target}\"\n")
    } else {
        format!("[[knobs]]\ninput = \"{address}\"\ncontrol = \"{target}\"\n")
    }
}
