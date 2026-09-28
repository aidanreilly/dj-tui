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
    assert_eq!(
        km.handle(KeyEvent::release(ch('c'))),
        Some(Action::CueRelease(A))
    );
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
    assert_eq!(
        km.handle(KeyEvent::release(ch('c'))),
        Some(Action::CueRelease(A))
    );
}

#[test]
fn digits_trigger_hot_cues_and_alt_digits_clear_them() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('1')), Some(Action::HotCue(A, 0)));
    assert_eq!(press(&mut km, ch('8')), Some(Action::HotCue(A, 7)));
    assert_eq!(
        km.handle(KeyEvent::press(ch('3')).alt()),
        Some(Action::ClearHotCue(A, 2))
    );
}

#[test]
fn the_band_keys_kill_and_swap() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('t')), Some(Action::EqKill(A, Band::High)));
    assert_eq!(press(&mut km, ch('y')), Some(Action::EqKill(A, Band::Mid)));
    assert_eq!(press(&mut km, ch('u')), Some(Action::EqKill(A, Band::Low)));
    assert_eq!(press(&mut km, ch('U')), Some(Action::EqSwap(A, Band::Low)));
    press(&mut km, Key::Tab);
    assert_eq!(press(&mut km, ch('T')), Some(Action::EqSwap(B, Band::High)));
}

#[test]
fn eq_gain_is_no_longer_on_the_keyboard() {
    let mut km = Keymap::new();
    for key in ['t', 'T', 'y', 'Y', 'u', 'U'] {
        let got = press(&mut km, ch(key));
        assert!(
            !matches!(got, Some(Action::Eq(..))),
            "{key} produced stepped EQ gain"
        );
    }
    assert_eq!(km.handle(KeyEvent::press(ch('t')).alt()), None);
}

#[test]
fn the_other_stepped_controls_keep_their_keys() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('r')), Some(Action::Trim(A, Dir::Down)));
    assert_eq!(press(&mut km, ch('R')), Some(Action::Trim(A, Dir::Up)));
    assert_eq!(press(&mut km, ch('o')), Some(Action::Filter(A, Dir::Down)));
    assert_eq!(press(&mut km, ch('O')), Some(Action::Filter(A, Dir::Up)));
}

#[test]
fn tempo_has_normal_and_fine_steps() {
    let mut km = Keymap::new();
    assert_eq!(
        press(&mut km, ch('+')),
        Some(Action::Tempo(A, Dir::Up, false))
    );
    assert_eq!(
        press(&mut km, ch('-')),
        Some(Action::Tempo(A, Dir::Down, false))
    );
    assert_eq!(
        km.handle(KeyEvent::press(ch('+')).alt()),
        Some(Action::Tempo(A, Dir::Up, true))
    );
}

#[test]
fn enter_loads_into_the_focused_deck() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, Key::Enter), Some(Action::Load(A)));
    press(&mut km, Key::Tab);
    assert_eq!(press(&mut km, Key::Enter), Some(Action::Load(B)));
}

#[test]
fn global_keys() {
    let mut km = Keymap::new();
    assert_eq!(
        press(&mut km, Key::Right),
        Some(Action::Crossfader(Dir::Up, false))
    );
    assert_eq!(
        km.handle(KeyEvent::press(Key::Left).shift()),
        Some(Action::Crossfader(Dir::Down, true))
    );
    // `/` and `b` hand the keyboard to the browser, which the modes tests cover, and the
    // arrows are the faders, which the arrow tests cover.
    assert_eq!(press(&mut km, ch('w')), Some(Action::CycleWaveformMode));
    assert_eq!(press(&mut km, ch('W')), Some(Action::CycleWaveformMode));
    assert_eq!(press(&mut km, ch('?')), Some(Action::Help));
    assert_eq!(
        km.handle(KeyEvent::press(ch('q')).ctrl()),
        Some(Action::Quit)
    );
}

#[test]
fn horizontal_arrows_are_the_crossfader() {
    let mut km = Keymap::new();
    assert_eq!(
        press(&mut km, Key::Right),
        Some(Action::Crossfader(Dir::Up, false))
    );
    assert_eq!(
        press(&mut km, Key::Left),
        Some(Action::Crossfader(Dir::Down, false))
    );
    assert_eq!(
        km.handle(KeyEvent::press(Key::Left).shift()),
        Some(Action::Crossfader(Dir::Down, true))
    );
    assert_eq!(press(&mut km, ch('x')), Some(Action::CrossfaderCentre));
}

#[test]
fn vertical_arrows_are_the_focused_decks_fader() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, Key::Up), Some(Action::Fader(A, Dir::Up)));
    assert_eq!(press(&mut km, Key::Down), Some(Action::Fader(A, Dir::Down)));
    assert_eq!(
        km.handle(KeyEvent::press(Key::Down).shift()),
        Some(Action::FaderEnd(A, Dir::Down))
    );
    press(&mut km, Key::Tab);
    assert_eq!(press(&mut km, Key::Up), Some(Action::Fader(B, Dir::Up)));
}

#[test]
fn v_is_the_filter_centre_and_no_longer_the_fader() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('v')), Some(Action::FilterCentre(A)));
    assert!(!matches!(press(&mut km, ch('V')), Some(Action::Fader(..))));
}

#[test]
fn the_browser_no_longer_moves_from_mix_mode() {
    let mut km = Keymap::new();
    assert!(!matches!(
        press(&mut km, Key::Up),
        Some(Action::BrowserMove(_))
    ));
    assert_eq!(
        press(&mut km, ch('S')),
        None,
        "sort is a browser command now"
    );
    assert_eq!(press(&mut km, ch('A')), None, "so is analysing");
}

#[test]
fn alt_on_a_continuous_control_fades_it() {
    let mut km = Keymap::new();
    let alt = |km: &mut Keymap, k: Key| km.handle(KeyEvent::press(k).alt());
    assert_eq!(
        alt(&mut km, Key::Left),
        Some(Action::CrossfaderFade(Dir::Down))
    );
    assert_eq!(
        alt(&mut km, Key::Right),
        Some(Action::CrossfaderFade(Dir::Up))
    );
    assert_eq!(alt(&mut km, Key::Up), Some(Action::FaderFade(A, Dir::Up)));
    assert_eq!(
        alt(&mut km, Key::Down),
        Some(Action::FaderFade(A, Dir::Down))
    );
    assert_eq!(
        km.handle(KeyEvent::press(ch('o')).alt()),
        Some(Action::FilterSweep(A, Dir::Down))
    );
    assert_eq!(
        km.handle(KeyEvent::press(ch('O')).alt()),
        Some(Action::FilterSweep(A, Dir::Up))
    );
}

#[test]
fn the_capitals_of_the_centre_keys_fade_to_centre() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('X')), Some(Action::CrossfaderFadeCentre));
    assert_eq!(press(&mut km, ch('V')), Some(Action::FilterSweepCentre(A)));
}

#[test]
fn braces_scale_the_fade_length_and_esc_cancels() {
    let mut km = Keymap::new();
    assert_eq!(press(&mut km, ch('{')), Some(Action::FadeLength(Dir::Down)));
    assert_eq!(press(&mut km, ch('}')), Some(Action::FadeLength(Dir::Up)));
    assert_eq!(press(&mut km, Key::Esc), Some(Action::CancelFades));
}

#[test]
fn deck_toggles_and_loops() {
    let mut km = Keymap::new();
    let d: DeckId = A;
    assert_eq!(press(&mut km, ch('s')), Some(Action::Sync(d)));
    assert_eq!(press(&mut km, ch('q')), Some(Action::Quantize(d)));
    assert_eq!(press(&mut km, ch('k')), Some(Action::KeyLock(d)));
    assert_eq!(press(&mut km, ch('l')), Some(Action::LoopToggle(d)));
    assert_eq!(press(&mut km, ch('i')), Some(Action::LoopIn(d)));
    assert_eq!(press(&mut km, ch('I')), Some(Action::LoopOut(d)));
    assert_eq!(press(&mut km, ch('[')), Some(Action::LoopHalve(d)));
    assert_eq!(press(&mut km, ch(']')), Some(Action::LoopDouble(d)));
    assert_eq!(
        press(&mut km, ch('<')),
        Some(Action::BeatJump(d, Dir::Down))
    );
    assert_eq!(press(&mut km, ch('.')), Some(Action::Nudge(d, Dir::Up)));
    assert_eq!(press(&mut km, ch('f')), Some(Action::FxToggle(d)));
    assert_eq!(press(&mut km, ch('F')), Some(Action::FxNext(d)));
    assert_eq!(
        press(&mut km, ch('p')),
        Some(Action::FxParam(d, 0, Dir::Down))
    );
    assert_eq!(
        press(&mut km, ch('P')),
        Some(Action::FxParam(d, 0, Dir::Up))
    );
    assert_eq!(
        press(&mut km, ch('D')),
        Some(Action::FxParam(d, 1, Dir::Up))
    );
    assert_eq!(press(&mut km, ch('0')), Some(Action::FxWet(d, Dir::Up)));
    assert_eq!(press(&mut km, ch('m')), Some(Action::HeadphoneCue(d)));
    assert_eq!(press(&mut km, ch('h')), Some(Action::CueMix(Dir::Down)));
    assert_eq!(press(&mut km, ch('H')), Some(Action::CueMix(Dir::Up)));
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

mod action_names {
    use engine::DeckId::{A, B};
    use input::{parse_action, Action, Band, Dir};

    #[test]
    fn transport_and_cues_read_as_words() {
        assert_eq!(parse_action("play a"), Some(Action::PlayPause(A)));
        assert_eq!(parse_action("cue b"), Some(Action::CuePress(B)));
        assert_eq!(parse_action("cue-release b"), Some(Action::CueRelease(B)));
        assert_eq!(parse_action("hotcue a 3"), Some(Action::HotCue(A, 2)));
        assert_eq!(parse_action("hotcue a 9"), None, "only eight pads");
        assert_eq!(parse_action("hotcue a"), None, "which pad?");
    }

    #[test]
    fn stepped_controls_name_their_direction() {
        assert_eq!(
            parse_action("tempo a up"),
            Some(Action::Tempo(A, Dir::Up, false))
        );
        assert_eq!(
            parse_action("tempo a down fine"),
            Some(Action::Tempo(A, Dir::Down, true))
        );
        assert_eq!(
            parse_action("eq b high up"),
            Some(Action::Eq(B, Band::High, Dir::Up))
        );
        assert_eq!(
            parse_action("eq b kill low"),
            Some(Action::EqKill(B, Band::Low))
        );
        assert_eq!(
            parse_action("crossfader up"),
            Some(Action::Crossfader(Dir::Up, false))
        );
        assert_eq!(
            parse_action("nudge a down"),
            Some(Action::Nudge(A, Dir::Down))
        );
        assert_eq!(
            parse_action("fx param a 2 up"),
            Some(Action::FxParam(A, 1, Dir::Up))
        );
    }

    #[test]
    fn the_toggles_need_no_more_than_a_deck() {
        assert_eq!(parse_action("sync a"), Some(Action::Sync(A)));
        assert_eq!(parse_action("keylock b"), Some(Action::KeyLock(B)));
        assert_eq!(parse_action("quantize a"), Some(Action::Quantize(A)));
        assert_eq!(parse_action("loop a"), Some(Action::LoopToggle(A)));
        assert_eq!(parse_action("loop-in a"), Some(Action::LoopIn(A)));
        assert_eq!(parse_action("fx a"), Some(Action::FxToggle(A)));
        assert_eq!(parse_action("fx-next a"), Some(Action::FxNext(A)));
        assert_eq!(parse_action("waveform"), Some(Action::CycleWaveformMode));
        assert_eq!(parse_action("cue-mix up"), Some(Action::CueMix(Dir::Up)));
        assert_eq!(parse_action("headphones a"), Some(Action::HeadphoneCue(A)));
    }

    #[test]
    fn the_new_gestures_have_names_a_mapping_can_use() {
        assert_eq!(
            parse_action("crossfader centre"),
            Some(Action::CrossfaderCentre)
        );
        assert_eq!(
            parse_action("filter a centre"),
            Some(Action::FilterCentre(A))
        );
        assert_eq!(
            parse_action("fader b down end"),
            Some(Action::FaderEnd(B, Dir::Down))
        );
        assert_eq!(
            parse_action("eq a swap low"),
            Some(Action::EqSwap(A, Band::Low))
        );
        assert_eq!(
            parse_action("crossfader fade up"),
            Some(Action::CrossfaderFade(Dir::Up))
        );
        assert_eq!(
            parse_action("crossfader fade centre"),
            Some(Action::CrossfaderFadeCentre)
        );
        assert_eq!(
            parse_action("fader a fade down"),
            Some(Action::FaderFade(A, Dir::Down))
        );
        assert_eq!(
            parse_action("filter b sweep up"),
            Some(Action::FilterSweep(B, Dir::Up))
        );
        assert_eq!(
            parse_action("filter b sweep centre"),
            Some(Action::FilterSweepCentre(B))
        );
        assert_eq!(
            parse_action("fade-length double"),
            Some(Action::FadeLength(Dir::Up))
        );
        assert_eq!(parse_action("cancel-fades"), Some(Action::CancelFades));
        assert_eq!(parse_action("browser"), Some(Action::BrowserEnter));
        assert_eq!(parse_action("browser-clear"), Some(Action::BrowserClear));
        // The general forms still reach their own arms.
        assert_eq!(
            parse_action("filter a up"),
            Some(Action::Filter(A, Dir::Up))
        );
        assert_eq!(
            parse_action("fader b down"),
            Some(Action::Fader(B, Dir::Down))
        );
        assert_eq!(
            parse_action("crossfader up"),
            Some(Action::Crossfader(Dir::Up, false))
        );
    }

    #[test]
    fn spelling_is_forgiving_but_nonsense_is_rejected() {
        assert_eq!(parse_action("  PLAY   A  "), Some(Action::PlayPause(A)));
        assert_eq!(parse_action("play c"), None, "there are two decks");
        assert_eq!(parse_action("fly a"), None);
        assert_eq!(parse_action(""), None);
        assert_eq!(parse_action("play a b"), None, "no trailing rubbish");
    }
}

mod modes {
    use super::*;
    use input::Mode;

    fn alt(km: &mut Keymap, key: Key) -> Option<Action> {
        km.handle(KeyEvent::press(key).alt())
    }

    #[test]
    fn mix_mode_is_where_it_starts() {
        assert_eq!(Keymap::new().mode(), Mode::Mix);
    }

    #[test]
    fn b_enters_browser_mode_and_esc_leaves_in_one_press() {
        let mut km = Keymap::new();
        assert_eq!(press(&mut km, ch('b')), Some(Action::BrowserEnter));
        assert_eq!(km.mode(), Mode::Browser);
        assert_eq!(press(&mut km, Key::Esc), Some(Action::BrowserLeave));
        assert_eq!(km.mode(), Mode::Mix);
    }

    #[test]
    fn slash_enters_browser_mode_clearing_the_query() {
        let mut km = Keymap::new();
        assert_eq!(press(&mut km, ch('/')), Some(Action::Search));
        assert_eq!(km.mode(), Mode::Browser);
    }

    #[test]
    fn letters_digits_and_space_type_in_browser_mode() {
        let mut km = Keymap::new();
        press(&mut km, ch('b'));
        assert_eq!(press(&mut km, ch('w')), Some(Action::BrowserType('w')));
        assert_eq!(press(&mut km, ch('4')), Some(Action::BrowserType('4')));
        assert_eq!(press(&mut km, Key::Space), Some(Action::BrowserType(' ')));
        assert_eq!(press(&mut km, ch('/')), Some(Action::BrowserType('/')));
        assert_eq!(press(&mut km, ch('b')), Some(Action::BrowserType('b')));
    }

    /// Review Focus 3. Shift plus a letter is a character, not a mix gesture.
    #[test]
    fn shift_and_a_letter_types_the_capital() {
        let mut km = Keymap::new();
        press(&mut km, ch('b'));
        assert_eq!(
            km.handle(KeyEvent::press(ch('B')).shift()),
            Some(Action::BrowserType('B'))
        );
        assert_eq!(
            km.handle(KeyEvent::press(ch('U')).shift()),
            Some(Action::BrowserType('U'))
        );
    }

    /// Review Focus 2. Browser mode owns the keyboard, so modified keys do not leak.
    #[test]
    fn modified_keys_do_not_reach_the_mixer_from_browser_mode() {
        let mut km = Keymap::new();
        press(&mut km, ch('b'));
        assert_eq!(alt(&mut km, Key::Left), None);
        assert_eq!(km.handle(KeyEvent::press(Key::Up).shift()), None);
        assert_eq!(km.handle(KeyEvent::press(ch('d')).ctrl()), None);
        assert_eq!(km.mode(), Mode::Browser, "none of that left the mode");
    }

    #[test]
    fn arrows_move_the_list_and_enter_loads_the_focused_deck() {
        let mut km = Keymap::new();
        press(&mut km, ch('b'));
        assert_eq!(
            press(&mut km, Key::Down),
            Some(Action::BrowserMove(Dir::Down))
        );
        assert_eq!(press(&mut km, Key::Up), Some(Action::BrowserMove(Dir::Up)));
        assert_eq!(press(&mut km, Key::Enter), Some(Action::Load(A)));
    }

    #[test]
    fn tab_stays_live_so_the_destination_deck_can_be_chosen() {
        let mut km = Keymap::new();
        press(&mut km, ch('b'));
        assert_eq!(press(&mut km, Key::Tab), Some(Action::Focus(B)));
        assert_eq!(km.mode(), Mode::Browser);
        assert_eq!(press(&mut km, Key::Enter), Some(Action::Load(B)));
    }

    #[test]
    fn backspace_and_ctrl_u_edit_the_query() {
        let mut km = Keymap::new();
        press(&mut km, ch('b'));
        assert_eq!(
            press(&mut km, Key::Backspace),
            Some(Action::BrowserBackspace)
        );
        assert_eq!(
            km.handle(KeyEvent::press(ch('u')).ctrl()),
            Some(Action::BrowserClear)
        );
    }

    #[test]
    fn alt_and_a_letter_is_a_browser_command() {
        let mut km = Keymap::new();
        press(&mut km, ch('b'));
        assert_eq!(alt(&mut km, ch('s')), Some(Action::BrowserSort(false)));
        assert_eq!(
            km.handle(KeyEvent::press(ch('S')).alt()),
            Some(Action::BrowserSort(true))
        );
        assert_eq!(alt(&mut km, ch('a')), Some(Action::AnalyseLibrary));
        assert_eq!(alt(&mut km, ch('f')), Some(Action::BrowserFullscreen));
    }

    #[test]
    fn ctrl_q_quits_from_any_mode() {
        let mut km = Keymap::new();
        press(&mut km, ch('b'));
        assert_eq!(
            km.handle(KeyEvent::press(ch('q')).ctrl()),
            Some(Action::Quit)
        );
    }

    #[test]
    fn ctrl_d_opens_and_closes_the_device_screen() {
        let mut km = Keymap::new();
        assert_eq!(
            km.handle(KeyEvent::press(ch('d')).ctrl()),
            Some(Action::Devices)
        );
        assert_eq!(km.mode(), Mode::Devices);
        assert_eq!(
            press(&mut km, Key::Down),
            Some(Action::DeviceMove(Dir::Down))
        );
        assert_eq!(press(&mut km, Key::Enter), Some(Action::DeviceChoose));
        assert_eq!(km.mode(), Mode::Mix, "choosing closes it");
        km.handle(KeyEvent::press(ch('d')).ctrl());
        assert_eq!(press(&mut km, Key::Esc), Some(Action::DeviceClose));
        assert_eq!(km.mode(), Mode::Mix);
    }

    #[test]
    fn letters_do_nothing_in_device_mode() {
        let mut km = Keymap::new();
        km.handle(KeyEvent::press(ch('d')).ctrl());
        assert_eq!(press(&mut km, ch('l')), None);
        assert_eq!(press(&mut km, Key::Space), None);
    }

    #[test]
    fn backtick_is_no_longer_a_key() {
        let mut km = Keymap::new();
        assert_eq!(press(&mut km, ch('`')), None);
        // The next key is an ordinary key on the focused deck, not a redirected one.
        assert_eq!(press(&mut km, Key::Space), Some(Action::PlayPause(A)));
    }
}

mod review_fixes {
    use super::*;

    #[test]
    fn tab_does_not_leave_the_seek_prefix_armed_on_the_deck_you_left() {
        let mut km = Keymap::new();
        press(&mut km, ch('g'));
        press(&mut km, Key::Tab);
        // Before this was fixed, the digit fired a seek on deck A while focus sat on B.
        let got = press(&mut km, ch('3'));
        assert!(
            !matches!(got, Some(Action::SeekTenth(..))),
            "the prefix leaked across Tab: {got:?}"
        );
    }

    #[test]
    fn holding_the_cue_key_presses_it_once() {
        let mut km = Keymap::new();
        assert_eq!(press(&mut km, ch('c')), Some(Action::CuePress(A)));
        // Auto-repeat arrives as more presses; the deck must not re-seek to the cue point.
        assert_eq!(press(&mut km, ch('c')), None);
        assert_eq!(press(&mut km, ch('c')), None);
        assert_eq!(
            km.handle(KeyEvent::release(ch('c'))),
            Some(Action::CueRelease(A))
        );
        // And it arms again after the release.
        assert_eq!(press(&mut km, ch('c')), Some(Action::CuePress(A)));
    }
}
