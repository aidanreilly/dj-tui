//! Focus-based keyboard mapping from spec section 3.8.

use engine::DeckId::{self, A, B};
use input::{Action, Band, Dir, Key, KeyEvent, Keymap};

fn press(km: &mut Keymap, key: Key) -> Option<Action> {
    km.handle(KeyEvent::press(key))
}
fn ch(c: char) -> Key {
    Key::Char(c)
}

#[test]
fn deck_a_is_focused_initially() {
    assert_eq!(Keymap::new().focused(), A);
}

#[test]
fn tab_toggles_focus_and_reports_it() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, Key::Tab), Some(Action::Focus(B)));
    assert_eq!(km.focused(), B);
    assert_eq!(press(&mut km, Key::Tab), Some(Action::Focus(A)));
}

#[test]
fn space_plays_the_focused_deck() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, Key::Space), Some(Action::PlayPause(A)));
    press(&mut km, Key::Tab);
    assert_eq!(press(&mut km, Key::Space), Some(Action::PlayPause(B)));
}

#[test]
fn cue_reports_press_and_release() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('c')), Some(Action::CuePress(A)));
    assert_eq!(km.handle(KeyEvent::release(ch('c'))), Some(Action::CueRelease(A)));
}

#[test]
fn releases_of_other_keys_do_nothing() {
    let mut km = Keymap::new();
    assert_eq!(km.handle(KeyEvent::release(Key::Space)), None);
    assert_eq!(km.handle(KeyEvent::release(ch('t'))), None);
}

#[test]
fn cue_release_goes_to_the_deck_that_was_pressed_even_after_focus_change() {
    let mut km = Keymap::new();
    press(&mut km, ch('c'));
    press(&mut km, Key::Tab);
    assert_eq!(km.handle(KeyEvent::release(ch('c'))), Some(Action::CueRelease(A)));
}

#[test]
fn digits_trigger_hot_cues_and_alt_digits_clear_them() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('1')), Some(Action::HotCue(A, 0)));
    assert_eq!(press(&mut km, ch('8')), Some(Action::HotCue(A, 7)));
    assert_eq!(km.handle(KeyEvent::press(ch('3')).alt()), Some(Action::ClearHotCue(A, 2)));
}

#[test]
fn lowercase_turns_down_and_uppercase_turns_up() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('t')), Some(Action::Eq(A, Band::High, Dir::Down)));
    assert_eq!(press(&mut km, ch('T')), Some(Action::Eq(A, Band::High, Dir::Up)));
    assert_eq!(press(&mut km, ch('u')), Some(Action::Eq(A, Band::Low, Dir::Down)));
    assert_eq!(press(&mut km, ch('v')), Some(Action::Fader(A, Dir::Down)));
    assert_eq!(press(&mut km, ch('V')), Some(Action::Fader(A, Dir::Up)));
    assert_eq!(press(&mut km, ch('r')), Some(Action::Trim(A, Dir::Down)));
    assert_eq!(press(&mut km, ch('O')), Some(Action::Filter(A, Dir::Up)));
}

#[test]
fn alt_on_eq_keys_toggles_kill() {
    let mut km = Keymap::new();
    assert_eq!(km.handle(KeyEvent::press(ch('y')).alt()), Some(Action::EqKill(A, Band::Mid)));
}

#[test]
fn tempo_has_normal_and_fine_steps() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('+')), Some(Action::Tempo(A, Dir::Up, false)));
    assert_eq!(press(&mut km, ch('-')), Some(Action::Tempo(A, Dir::Down, false)));
    assert_eq!(km.handle(KeyEvent::press(ch('+')).alt()), Some(Action::Tempo(A, Dir::Up, true)));
}

#[test]
fn backtick_sends_exactly_one_key_to_the_other_deck() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('`')), None);
    assert_eq!(press(&mut km, ch('u')), Some(Action::Eq(B, Band::Low, Dir::Down)));
    assert_eq!(km.focused(), A);
    assert_eq!(press(&mut km, ch('u')), Some(Action::Eq(A, Band::Low, Dir::Down)));
}

#[test]
fn backtick_before_a_global_key_is_consumed() {
    let mut km = Keymap::new();
    press(&mut km, ch('`'));
    assert_eq!(press(&mut km, Key::Left), Some(Action::Crossfader(Dir::Down, false)));
    assert_eq!(press(&mut km, Key::Space), Some(Action::PlayPause(A)));
}

#[test]
fn double_backtick_cancels() {
    let mut km = Keymap::new();
    press(&mut km, ch('`'));
    press(&mut km, ch('`'));
    assert_eq!(press(&mut km, Key::Space), Some(Action::PlayPause(A)));
}

#[test]
fn enter_loads_into_focused_or_other_deck() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, Key::Enter), Some(Action::Load(A)));
    press(&mut km, ch('`'));
    assert_eq!(press(&mut km, Key::Enter), Some(Action::Load(B)));
}

#[test]
fn global_keys() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, Key::Right), Some(Action::Crossfader(Dir::Up, false)));
    assert_eq!(km.handle(KeyEvent::press(Key::Left).shift()), Some(Action::Crossfader(Dir::Down, true)));
    assert_eq!(press(&mut km, Key::Up), Some(Action::BrowserMove(Dir::Up)));
    assert_eq!(press(&mut km, ch('/')), Some(Action::Search));
    assert_eq!(press(&mut km, ch('b')), Some(Action::BrowserFullscreen));
    assert_eq!(press(&mut km, ch('W')), Some(Action::CycleWaveformMode));
    assert_eq!(press(&mut km, ch('?')), Some(Action::Help));
    assert_eq!(km.handle(KeyEvent::press(ch('q')).ctrl()), Some(Action::Quit));
}

#[test]
fn deck_toggles_and_loops() {
    let mut km = Keymap::new();
    let d: DeckId = A;
    assert_eq!(press(&mut km, ch('s')), Some(Action::Sync(d)));
    assert_eq!(press(&mut km, ch('q')), Some(Action::Quantize(d)));
    assert_eq!(press(&mut km, ch('k')), Some(Action::KeyLock(d)));
    assert_eq!(press(&mut km, ch('l')), Some(Action::LoopToggle(d)));
    assert_eq!(press(&mut km, ch('[')), Some(Action::LoopHalve(d)));
    assert_eq!(press(&mut km, ch(']')), Some(Action::LoopDouble(d)));
    assert_eq!(press(&mut km, ch('<')), Some(Action::BeatJump(d, Dir::Down)));
    assert_eq!(press(&mut km, ch('.')), Some(Action::Nudge(d, Dir::Up)));
    assert_eq!(press(&mut km, ch('f')), Some(Action::FxToggle(d)));
    assert_eq!(press(&mut km, ch('F')), Some(Action::FxNext(d)));
    assert_eq!(press(&mut km, ch('0')), Some(Action::FxWet(d, Dir::Up)));
    assert_eq!(press(&mut km, ch('m')), Some(Action::HeadphoneCue(d)));
}

#[test]
fn g_then_digit_seeks_to_a_tenth() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('g')), None);
    assert_eq!(press(&mut km, ch('5')), Some(Action::SeekTenth(A, 5)));
    // The digit after a completed seek is a hot cue again.
    assert_eq!(press(&mut km, ch('5')), Some(Action::HotCue(A, 4)));
}

#[test]
fn g_followed_by_a_non_digit_is_dropped() {
    let mut km = Keymap::new();
    press(&mut km, ch('g'));
    assert_eq!(press(&mut km, ch('x')), None);
    assert_eq!(press(&mut km, Key::Space), Some(Action::PlayPause(A)));
}

#[test]
fn unmapped_keys_do_nothing() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('z')), None);
}
