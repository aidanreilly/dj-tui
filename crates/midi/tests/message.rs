//! Reading and writing the wire format.

use midi::{Address, Message};

#[test]
fn the_four_message_types_read_off_the_wire() {
    assert_eq!(
        Message::from_bytes(&[0x90, 36, 100]),
        Some(Message::NoteOn {
            channel: 0,
            note: 36,
            velocity: 100
        })
    );
    assert_eq!(
        Message::from_bytes(&[0x85, 36, 0]),
        Some(Message::NoteOff {
            channel: 5,
            note: 36,
            velocity: 0
        })
    );
    assert_eq!(
        Message::from_bytes(&[0xb2, 16, 64]),
        Some(Message::Cc {
            channel: 2,
            controller: 16,
            value: 64
        })
    );
    assert_eq!(
        Message::from_bytes(&[0xe0, 0x00, 0x40]),
        Some(Message::PitchBend {
            channel: 0,
            value: 8_192
        }),
        "the low seven bits come first"
    );
}

#[test]
fn anything_else_on_the_wire_is_dropped() {
    assert_eq!(Message::from_bytes(&[]), None);
    assert_eq!(Message::from_bytes(&[0xf8]), None, "clock");
    assert_eq!(Message::from_bytes(&[0xf0, 1, 2]), None, "sysex");
    assert_eq!(Message::from_bytes(&[0x90, 36]), None, "cut short");
    assert_eq!(
        Message::from_bytes(&[0x90, 0x90, 1]),
        None,
        "a status byte where data belongs"
    );
    assert_eq!(Message::from_bytes(&[36, 100]), None, "running status");
}

#[test]
fn what_is_read_can_be_written_back() {
    for m in [
        Message::NoteOn {
            channel: 3,
            note: 60,
            velocity: 127,
        },
        Message::NoteOff {
            channel: 0,
            note: 60,
            velocity: 0,
        },
        Message::Cc {
            channel: 15,
            controller: 7,
            value: 100,
        },
        Message::PitchBend {
            channel: 1,
            value: 12_345,
        },
    ] {
        assert_eq!(Message::from_bytes(&m.to_bytes()), Some(m), "{m:?}");
    }
}

#[test]
fn a_message_knows_the_address_a_mapping_would_call_it() {
    let note = Message::NoteOn {
        channel: 2,
        note: 48,
        velocity: 1,
    };
    assert_eq!(note.address(), "note 2 48");
    assert_eq!(Address::parse("note 2 48").unwrap(), Address::of(note));
    assert_eq!(
        Message::Cc {
            channel: 0,
            controller: 16,
            value: 0
        }
        .address(),
        "cc 0 16"
    );
    assert!(Address::parse("cc 0 200").is_err(), "out of range");
    assert!(Address::parse("note 0").is_err(), "which note?");
}
