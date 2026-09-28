//! Continuous control identity, shared by the keyboard's fades and by MIDI knobs.

use engine::DeckId::{A, B};
use input::{Band, Control};

#[test]
fn controls_read_as_words() {
    assert_eq!(Control::parse("crossfader"), Some(Control::Crossfader));
    assert_eq!(Control::parse("cue-mix"), Some(Control::CueMix));
    assert_eq!(Control::parse("fader a"), Some(Control::Fader(A)));
    assert_eq!(Control::parse("tempo b"), Some(Control::Tempo(B)));
    assert_eq!(Control::parse("trim a"), Some(Control::Trim(A)));
    assert_eq!(Control::parse("filter b"), Some(Control::Filter(B)));
    assert_eq!(Control::parse("eq b mid"), Some(Control::Eq(B, Band::Mid)));
}

#[test]
fn nonsense_is_rejected_rather_than_guessed_at() {
    assert_eq!(Control::parse("fader c"), None);
    assert_eq!(Control::parse("eq a sideways"), None);
    assert_eq!(Control::parse(""), None);
}

#[test]
fn effect_controls_are_rejected() {
    assert_eq!(Control::parse("fx wet a"), None);
    assert_eq!(Control::parse("fx param a 1"), None);
}

#[test]
fn the_midi_crate_still_exposes_it_under_the_old_path() {
    assert_eq!(
        midi::Control::parse("crossfader"),
        Some(Control::Crossfader)
    );
}
