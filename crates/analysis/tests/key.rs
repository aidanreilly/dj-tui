//! Musical key from a chromagram and key profiles, shown in Camelot notation.

mod common;
use analysis::key::{detect_key, Key, Mode, PitchClass};
use common::*;

fn camelot(t: &engine::Track) -> String {
    detect_key(t).expect("key").camelot()
}

#[test]
fn a_minor_chord_with_bass() {
    // A2 bass, A3 C4 E4.
    let t = chord(&[midi(45), midi(57), midi(60), midi(64)], 8.0);
    assert_eq!(camelot(&t), "8A");
}

#[test]
fn c_major_chord_with_bass() {
    let t = chord(&[midi(36), midi(60), midi(64), midi(67)], 8.0);
    assert_eq!(camelot(&t), "8B");
}

#[test]
fn f_sharp_minor_and_e_flat_major() {
    assert_eq!(
        camelot(&chord(&[midi(42), midi(54), midi(57), midi(61)], 8.0)),
        "11A"
    );
    assert_eq!(
        camelot(&chord(&[midi(39), midi(51), midi(55), midi(58)], 8.0)),
        "5B"
    );
}

#[test]
fn silence_has_no_key() {
    assert!(detect_key(&silence(5.0)).is_none());
}

#[test]
fn camelot_wheel_is_complete_and_correct() {
    let all: std::collections::HashSet<String> = (0..12)
        .flat_map(|pc| {
            [Mode::Major, Mode::Minor].map(move |m| {
                Key {
                    tonic: PitchClass(pc),
                    mode: m,
                }
                .camelot()
            })
        })
        .collect();
    assert_eq!(all.len(), 24);
    let k = |pc: u8, mode| {
        Key {
            tonic: PitchClass(pc),
            mode,
        }
        .camelot()
    };
    assert_eq!(k(9, Mode::Minor), "8A");
    assert_eq!(k(0, Mode::Major), "8B");
    assert_eq!(k(4, Mode::Minor), "9A");
    assert_eq!(k(7, Mode::Major), "9B");
    assert_eq!(k(11, Mode::Major), "1B");
    assert_eq!(k(8, Mode::Minor), "1A");
}

#[test]
fn standard_names() {
    assert_eq!(
        Key {
            tonic: PitchClass(9),
            mode: Mode::Minor
        }
        .name(),
        "A minor"
    );
    assert_eq!(
        Key {
            tonic: PitchClass(3),
            mode: Mode::Major
        }
        .name(),
        "E♭ major"
    );
    assert_eq!(
        Key {
            tonic: PitchClass(6),
            mode: Mode::Minor
        }
        .name(),
        "F♯ minor"
    );
}
