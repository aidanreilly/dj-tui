//! MIDI messages, as much of them as a controller mapping needs.

/// One channel-voice message. System messages are dropped before they reach here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    NoteOff {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    Cc {
        channel: u8,
        controller: u8,
        value: u8,
    },
    /// Fourteen bits, 0 to 16383, centre at 8192.
    PitchBend {
        channel: u8,
        value: u16,
    },
}

impl Message {
    /// Read one message off the wire. Running status is not handled: every controller this
    /// targets sends whole messages, and a partial one is better dropped than guessed at.
    pub fn from_bytes(bytes: &[u8]) -> Option<Message> {
        let status = *bytes.first()?;
        let channel = status & 0x0f;
        let data = |i: usize| bytes.get(i).copied().filter(|b| *b < 0x80);
        match status & 0xf0 {
            0x80 => Some(Message::NoteOff {
                channel,
                note: data(1)?,
                velocity: data(2)?,
            }),
            0x90 => Some(Message::NoteOn {
                channel,
                note: data(1)?,
                velocity: data(2)?,
            }),
            0xb0 => Some(Message::Cc {
                channel,
                controller: data(1)?,
                value: data(2)?,
            }),
            0xe0 => Some(Message::PitchBend {
                channel,
                value: data(1)? as u16 | ((data(2)? as u16) << 7),
            }),
            _ => None,
        }
    }

    pub fn to_bytes(self) -> [u8; 3] {
        match self {
            Message::NoteOff {
                channel,
                note,
                velocity,
            } => [0x80 | channel, note, velocity],
            Message::NoteOn {
                channel,
                note,
                velocity,
            } => [0x90 | channel, note, velocity],
            Message::Cc {
                channel,
                controller,
                value,
            } => [0xb0 | channel, controller, value],
            Message::PitchBend { channel, value } => {
                [0xe0 | channel, (value & 0x7f) as u8, (value >> 7) as u8]
            }
        }
    }

    /// How this message is written in a mapping file, without its value.
    pub fn address(self) -> String {
        match self {
            Message::NoteOn { channel, note, .. } | Message::NoteOff { channel, note, .. } => {
                format!("note {channel} {note}")
            }
            Message::Cc {
                channel,
                controller,
                ..
            } => format!("cc {channel} {controller}"),
            Message::PitchBend { channel, .. } => format!("pitchbend {channel}"),
        }
    }
}

/// What a mapping file's `input` and `output` fields name: a message without its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Address {
    Note { channel: u8, note: u8 },
    Cc { channel: u8, controller: u8 },
    PitchBend { channel: u8 },
}

impl Address {
    /// Parse `note 0 36`, `cc 0 16` or `pitchbend 0`.
    pub fn parse(text: &str) -> Result<Address, String> {
        Self::parse_inner(text).ok_or_else(|| {
            format!("{text:?} is not a MIDI address like \"note 0 36\" or \"cc 0 16\"")
        })
    }

    fn parse_inner(text: &str) -> Option<Address> {
        let lower = text.to_ascii_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        let number = |w: &str| w.parse::<u8>().ok().filter(|n| *n < 128);
        match words.as_slice() {
            ["note", ch, note] => Some(Address::Note {
                channel: number(ch)? % 16,
                note: number(note)?,
            }),
            ["cc", ch, cc] => Some(Address::Cc {
                channel: number(ch)? % 16,
                controller: number(cc)?,
            }),
            ["pitchbend", ch] => Some(Address::PitchBend {
                channel: number(ch)? % 16,
            }),
            _ => None,
        }
    }

    pub fn of(message: Message) -> Address {
        match message {
            Message::NoteOn { channel, note, .. } | Message::NoteOff { channel, note, .. } => {
                Address::Note { channel, note }
            }
            Message::Cc {
                channel,
                controller,
                ..
            } => Address::Cc {
                channel,
                controller,
            },
            Message::PitchBend { channel, .. } => Address::PitchBend { channel },
        }
    }
}
