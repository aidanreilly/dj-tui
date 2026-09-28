//! What scheduling the audio thread actually got. A misconfigured system leaves it competing
//! with everything else, which is heard as glitches and is otherwise invisible.

use backend::realtime::{self, Scheduling, AUDIO_PRIORITY};

#[test]
fn an_ordinary_thread_is_not_realtime() {
    assert_eq!(realtime::scheduling(), Scheduling::Other);
    assert!(!Scheduling::Other.is_realtime());
}

#[test]
fn realtime_classes_say_so_and_carry_their_priority() {
    assert!(Scheduling::Fifo(10).is_realtime());
    assert!(Scheduling::RoundRobin(10).is_realtime());
    assert_eq!(Scheduling::Fifo(10).priority(), Some(10));
    assert_eq!(Scheduling::Other.priority(), None);
}

#[test]
fn each_class_has_a_label_for_the_status_line() {
    assert_eq!(Scheduling::Fifo(10).label(), "realtime FIFO 10");
    assert_eq!(Scheduling::RoundRobin(7).label(), "realtime RR 7");
    assert_eq!(Scheduling::Other.label(), "not realtime");
}

#[test]
fn a_class_survives_a_round_trip_through_an_atomic() {
    for s in [
        Scheduling::Other,
        Scheduling::Fifo(1),
        Scheduling::Fifo(AUDIO_PRIORITY),
        Scheduling::Fifo(99),
        Scheduling::RoundRobin(42),
    ] {
        assert_eq!(Scheduling::from_bits(s.to_bits()), Some(s), "{s:?}");
    }
    // Zero is what an atomic holds before the audio thread has reported anything.
    assert_eq!(Scheduling::from_bits(0), None);
}

/// Asking is allowed to fail: an unprivileged user cannot take SCHED_FIFO, and that is the
/// normal case. What must hold is that the answer and the thread agree afterwards, and that
/// a refusal explains itself rather than being silent.
#[test]
fn requesting_realtime_either_takes_effect_or_explains_itself() {
    let before = realtime::scheduling();
    assert_eq!(before, Scheduling::Other, "a test thread starts ordinary");
    match realtime::request_realtime(AUDIO_PRIORITY) {
        Ok(got) => {
            assert!(
                got.is_realtime(),
                "reported success without realtime: {got:?}"
            );
            assert_eq!(realtime::scheduling(), got, "the thread disagrees");
        }
        Err(why) => {
            assert!(!why.is_empty(), "a refusal has to say why");
            assert_eq!(
                realtime::scheduling(),
                Scheduling::Other,
                "a refusal left the thread changed"
            );
        }
    }
}

#[test]
fn the_priority_asked_for_is_modest() {
    // High enough to beat ordinary work, low enough to leave the kernel's own threads alone.
    assert!(
        (1..=20).contains(&AUDIO_PRIORITY),
        "AUDIO_PRIORITY is {AUDIO_PRIORITY}"
    );
}
