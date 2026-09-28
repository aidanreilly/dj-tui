//! Controller mappings: MIDI in, actions and absolute control moves out.

use engine::DeckId::{A, B};
use input::{Action, Band, Dir};
use midi::{Control, Mapping, Message, Outcome};

const SAMPLE: &str = r#"
name = "Example 2-deck"
ports = ["Example Controller", "EXAMPLE MIDI"]

[[buttons]]
input = "note 0 36"
action = "play a"

[[buttons]]
input = "note 1 36"
action = "play b"

[[buttons]]
input = "note 0 37"
action = "cue a"
hold = true

[[knobs]]
input = "cc 0 16"
control = "fader a"

[[knobs]]
input = "pitchbend 0"
control = "tempo a"

[[encoders]]
input = "cc 0 20"
action = "beatjump a"

[[leds]]
output = "note 0 36"
state = "playing a"
"#;

fn mapping() -> Mapping {
    Mapping::from_toml(SAMPLE).expect("the example mapping parses")
}

#[test]
fn a_mapping_says_what_it_is_and_which_ports_it_wants() {
    let m = mapping();
    assert_eq!(m.name(), "Example 2-deck");
    assert!(m.matches_port("Example Controller:0"));
    assert!(m.matches_port("example controller"), "case does not matter");
    assert!(!m.matches_port("Some Other Keyboard"));
}

#[test]
fn a_note_press_gives_its_action_and_the_release_gives_nothing() {
    let mut m = mapping();
    assert_eq!(
        m.handle(Message::NoteOn {
            channel: 0,
            note: 36,
            velocity: 100
        }),
        Outcome::Act(Action::PlayPause(A))
    );
    assert_eq!(
        m.handle(Message::NoteOff {
            channel: 0,
            note: 36,
            velocity: 0
        }),
        Outcome::Nothing,
        "a plain button does nothing on release"
    );
    assert_eq!(
        m.handle(Message::NoteOn {
            channel: 1,
            note: 36,
            velocity: 100
        }),
        Outcome::Act(Action::PlayPause(B)),
        "the channel picks the deck out"
    );
}

#[test]
fn a_note_on_at_zero_velocity_is_a_release_like_the_spec_allows() {
    let mut m = mapping();
    assert_eq!(
        m.handle(Message::NoteOn {
            channel: 0,
            note: 36,
            velocity: 0
        }),
        Outcome::Nothing
    );
}

#[test]
fn a_held_button_reports_press_and_release() {
    let mut m = mapping();
    assert_eq!(
        m.handle(Message::NoteOn {
            channel: 0,
            note: 37,
            velocity: 64
        }),
        Outcome::Act(Action::CuePress(A))
    );
    assert_eq!(
        m.handle(Message::NoteOff {
            channel: 0,
            note: 37,
            velocity: 0
        }),
        Outcome::Act(Action::CueRelease(A))
    );
}

#[test]
fn an_unmapped_message_is_ignored() {
    let mut m = mapping();
    assert_eq!(
        m.handle(Message::Cc {
            channel: 5,
            controller: 99,
            value: 1
        }),
        Outcome::Nothing
    );
}

#[test]
fn a_knob_reports_where_it_was_moved_to() {
    let mut m = mapping();
    m.engage_all();
    assert_eq!(
        m.handle(Message::Cc {
            channel: 0,
            controller: 16,
            value: 127
        }),
        Outcome::Set(Control::Fader(A), 1.0)
    );
    match m.handle(Message::Cc {
        channel: 0,
        controller: 16,
        value: 64,
    }) {
        Outcome::Set(Control::Fader(A), v) => assert!((v - 0.504).abs() < 0.01, "{v}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn pitch_bend_reads_as_a_fourteen_bit_knob() {
    let mut m = mapping();
    m.engage_all();
    assert_eq!(
        m.handle(Message::PitchBend {
            channel: 0,
            value: 16_383
        }),
        Outcome::Set(Control::Tempo(A), 1.0)
    );
    match m.handle(Message::PitchBend {
        channel: 0,
        value: 8_192,
    }) {
        Outcome::Set(Control::Tempo(A), v) => assert!((v - 0.5).abs() < 0.001, "{v}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn soft_takeover_waits_for_the_knob_to_reach_the_value_on_screen() {
    let mut m = mapping();
    // The fader on screen is at 0.8; the controller's is somewhere near the bottom.
    m.sync_control(Control::Fader(A), 0.8);
    assert_eq!(
        m.handle(Message::Cc {
            channel: 0,
            controller: 16,
            value: 10
        }),
        Outcome::Nothing,
        "moving it down there would jump the sound"
    );
    assert_eq!(
        m.handle(Message::Cc {
            channel: 0,
            controller: 16,
            value: 90
        }),
        Outcome::Nothing,
        "still short of it"
    );
    match m.handle(Message::Cc {
        channel: 0,
        controller: 16,
        value: 103,
    }) {
        // 103 of 127 is 0.81, which has caught up with the 0.8 on screen.
        Outcome::Set(Control::Fader(A), v) => assert!(v > 0.79, "{v}"),
        other => panic!("it picks up once it catches up, got {other:?}"),
    }
    match m.handle(Message::Cc {
        channel: 0,
        controller: 16,
        value: 20,
    }) {
        Outcome::Set(Control::Fader(A), v) => assert!(v < 0.2, "{v}"),
        other => panic!("and follows freely after that, got {other:?}"),
    }
}

#[test]
fn an_encoder_reads_as_a_direction_either_way_round() {
    let mut m = mapping();
    // Two's complement: 1 is one step up, 127 is one step down.
    assert_eq!(
        m.handle(Message::Cc {
            channel: 0,
            controller: 20,
            value: 1
        }),
        Outcome::Act(Action::BeatJump(A, Dir::Up))
    );
    assert_eq!(
        m.handle(Message::Cc {
            channel: 0,
            controller: 20,
            value: 127
        }),
        Outcome::Act(Action::BeatJump(A, Dir::Down))
    );
    assert_eq!(
        m.handle(Message::Cc {
            channel: 0,
            controller: 20,
            value: 0
        }),
        Outcome::Nothing,
        "a resting encoder says nothing"
    );
}

#[test]
fn control_names_cover_the_mixer_and_the_decks() {
    let m = Mapping::from_toml(
        r#"
name = "Knobs"
[[knobs]]
input = "cc 0 1"
control = "crossfader"
[[knobs]]
input = "cc 0 2"
control = "eq b mid"
[[knobs]]
input = "cc 0 3"
control = "fx wet b"
[[knobs]]
input = "cc 0 4"
control = "fx param a 2"
"#,
    )
    .unwrap();
    assert_eq!(m.control_for("cc 0 1"), Some(Control::Crossfader));
    assert_eq!(m.control_for("cc 0 2"), Some(Control::Eq(B, Band::Mid)));
    assert_eq!(m.control_for("cc 0 3"), Some(Control::FxWet(B)));
    assert_eq!(m.control_for("cc 0 4"), Some(Control::FxParam(A, 1)));
}

#[test]
fn a_mapping_file_with_a_typo_says_what_is_wrong() {
    let err = Mapping::from_toml(
        r#"
name = "Broken"
[[buttons]]
input = "note 0 36"
action = "fly a"
"#,
    )
    .unwrap_err();
    assert!(err.contains("fly a"), "{err}");

    let err = Mapping::from_toml(
        r#"
name = "Broken"
[[buttons]]
input = "trumpet 0 36"
action = "play a"
"#,
    )
    .unwrap_err();
    assert!(err.contains("trumpet"), "{err}");
}

#[test]
fn learn_mode_turns_the_next_message_into_a_line_for_the_file() {
    assert_eq!(
        midi::learn_line(
            Message::Cc {
                channel: 2,
                controller: 34,
                value: 64
            },
            "fader b"
        ),
        "[[knobs]]\ninput = \"cc 2 34\"\ncontrol = \"fader b\"\n"
    );
    assert_eq!(
        midi::learn_line(
            Message::NoteOn {
                channel: 0,
                note: 48,
                velocity: 127
            },
            "play a"
        ),
        "[[buttons]]\ninput = \"note 0 48\"\naction = \"play a\"\n"
    );
}

/// Every action the keyboard's new gestures produce, as a mapping file names them. An encoder
/// gets its direction appended, so its action has to parse with `up` and `down` on the end.
mod new_gestures {
    use super::*;

    const HEAD: &str = "name = \"Probe\"\nports = [\"Probe\"]\n";

    #[test]
    fn the_shipped_examples_parse_as_buttons() {
        for name in [
            "crossfader fade up",
            "crossfader fade down",
            "crossfader fade centre",
            "crossfader centre",
            "fader a fade down",
            "fader b fade up",
            "filter a sweep up",
            "filter a sweep centre",
            "filter b centre",
            "cancel-fades",
            "fade-length halve",
            "fade-length double",
        ] {
            let toml = format!("{HEAD}[[buttons]]\ninput = \"note 0 11\"\naction = \"{name}\"\n");
            Mapping::from_toml(&toml).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn the_fade_length_encoder_parses_with_a_direction_appended() {
        let m = Mapping::from_toml(&format!(
            "{HEAD}[[encoders]]\ninput = \"cc 0 40\"\naction = \"fade-length\"\n"
        ))
        .expect("an encoder turns the fade length both ways");
        let mut m = m;
        let turn = |m: &mut Mapping, value: u8| {
            m.handle(Message::Cc {
                channel: 0,
                controller: 40,
                value,
            })
        };
        // Two's complement around 64, as the rest of this crate reads encoders.
        assert_eq!(
            turn(&mut m, 1),
            Outcome::Act(Action::FadeLength(Dir::Up)),
            "one way doubles it"
        );
        assert_eq!(
            turn(&mut m, 127),
            Outcome::Act(Action::FadeLength(Dir::Down)),
            "the other halves it"
        );
    }

    #[test]
    fn the_sweep_and_fade_gestures_work_as_encoders_too() {
        for name in ["crossfader fade", "fader a fade", "filter b sweep"] {
            let toml = format!("{HEAD}[[encoders]]\ninput = \"cc 0 41\"\naction = \"{name}\"\n");
            Mapping::from_toml(&toml).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }
}
