//! Lighting the controller up from what the app is doing.

use engine::DeckId::{A, B};
use midi::{Feedback, LedState, Mapping, Message};

const SAMPLE: &str = r#"
name = "Lit up"
[[leds]]
output = "note 0 36"
state = "playing a"
[[leds]]
output = "note 1 36"
state = "playing b"
[[leds]]
output = "cc 0 60"
state = "loop a"
"#;

#[test]
fn led_states_name_what_the_deck_is_doing() {
    assert_eq!(LedState::parse("playing a"), Some(LedState::Playing(A)));
    assert_eq!(LedState::parse("cued b"), Some(LedState::Cued(B)));
    assert_eq!(LedState::parse("loop a"), Some(LedState::Loop(A)));
    assert_eq!(LedState::parse("fx a"), Some(LedState::Fx(A)));
    assert_eq!(LedState::parse("sync a"), Some(LedState::Sync(A)));
    assert_eq!(LedState::parse("keylock b"), Some(LedState::KeyLock(B)));
    assert_eq!(LedState::parse("hotcue a 4"), Some(LedState::HotCue(A, 3)));
    assert_eq!(LedState::parse("dancing a"), None);
}

#[test]
fn a_mapping_rejects_an_led_state_it_cannot_follow() {
    let err = Mapping::from_toml(
        r#"
name = "Broken"
[[leds]]
output = "note 0 36"
state = "dancing a"
"#,
    )
    .unwrap_err();
    assert!(err.contains("dancing a"), "{err}");
}

#[test]
fn only_what_changed_goes_out() {
    let mapping = Mapping::from_toml(SAMPLE).unwrap();
    let mut feedback = Feedback::new(&mapping);
    let mut playing_a = false;

    let first = feedback.update(|state| match state {
        LedState::Playing(A) => playing_a,
        _ => false,
    });
    assert_eq!(first.len(), 3, "every light is set once at the start");

    let again = feedback.update(|state| match state {
        LedState::Playing(A) => playing_a,
        _ => false,
    });
    assert!(again.is_empty(), "nothing changed, so nothing is sent");

    playing_a = true;
    let lit = feedback.update(|state| match state {
        LedState::Playing(A) => playing_a,
        _ => false,
    });
    assert_eq!(
        lit,
        vec![Message::NoteOn {
            channel: 0,
            note: 36,
            velocity: 127
        }]
    );
}

#[test]
fn a_light_that_goes_out_is_sent_as_off() {
    let mapping = Mapping::from_toml(SAMPLE).unwrap();
    let mut feedback = Feedback::new(&mapping);
    feedback.update(|_| true);
    let dark = feedback.update(|_| false);
    assert!(dark.contains(&Message::NoteOn {
        channel: 0,
        note: 36,
        velocity: 0
    }));
    assert!(
        dark.contains(&Message::Cc {
            channel: 0,
            controller: 60,
            value: 0
        }),
        "a controller lit from a CC goes out the same way: {dark:?}"
    );
}
