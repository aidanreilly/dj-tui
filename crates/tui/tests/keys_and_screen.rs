use engine::DeckId;
use input::{Key, KeyEvent};
use ratatui::crossterm::event::{
    KeyCode, KeyEvent as CtKey, KeyEventKind, KeyEventState, KeyModifiers,
};
use ratatui::{backend::TestBackend, Terminal};
use tui::{convert_key, render_screen, DeckView, MixerView, ScreenView};

fn ct(code: KeyCode, mods: KeyModifiers, kind: KeyEventKind) -> CtKey {
    CtKey {
        code,
        modifiers: mods,
        kind,
        state: KeyEventState::NONE,
    }
}

#[test]
fn converts_plain_characters() {
    let e = convert_key(ct(
        KeyCode::Char('t'),
        KeyModifiers::NONE,
        KeyEventKind::Press,
    ));
    assert_eq!(e, Some(KeyEvent::press(Key::Char('t'))));
}

#[test]
fn converts_modifiers_and_release() {
    let e = convert_key(ct(
        KeyCode::Char('3'),
        KeyModifiers::ALT,
        KeyEventKind::Press,
    ));
    assert_eq!(e, Some(KeyEvent::press(Key::Char('3')).alt()));
    let e = convert_key(ct(
        KeyCode::Char('c'),
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));
    assert_eq!(e, Some(KeyEvent::release(Key::Char('c'))));
    let e = convert_key(ct(KeyCode::Left, KeyModifiers::SHIFT, KeyEventKind::Press));
    assert_eq!(e, Some(KeyEvent::press(Key::Left).shift()));
}

#[test]
fn space_is_its_own_key_and_backtab_is_tab() {
    let e = convert_key(ct(
        KeyCode::Char(' '),
        KeyModifiers::NONE,
        KeyEventKind::Press,
    ));
    assert_eq!(e, Some(KeyEvent::press(Key::Space)));
    let e = convert_key(ct(
        KeyCode::BackTab,
        KeyModifiers::SHIFT,
        KeyEventKind::Press,
    ));
    assert_eq!(e.map(|e| e.key), Some(Key::Tab));
}

#[test]
fn key_repeat_is_ignored_so_holds_do_not_retrigger() {
    let e = convert_key(ct(
        KeyCode::Char('c'),
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ));
    assert_eq!(e, None);
}

#[test]
fn unsupported_keys_are_dropped() {
    assert_eq!(
        convert_key(ct(KeyCode::F(5), KeyModifiers::NONE, KeyEventKind::Press)),
        None
    );
}

fn deck(id: DeckId, focused: bool) -> DeckView {
    DeckView {
        id,
        focused,
        title: None,
        bpm: None,
        key: None,
        position_secs: 0.0,
        duration_secs: 0.0,
        loading: false,
        playing: false,
        hot_cues: [false; 8],
        waveform: vec![],
    }
}

#[test]
fn whole_screen_shows_every_section() {
    let view = ScreenView {
        decks: [deck(DeckId::A, true), deck(DeckId::B, false)],
        mixer: MixerView {
            crossfader: -1.0,
            faders: [1.0, 0.5],
            headphone_cue: [false, true],
            ..Default::default()
        },
        status: "keyboard: kitty protocol".into(),
        message: "Loaded Some Track on deck A".into(),
    };
    let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
    term.draw(|f| render_screen(f, &view)).unwrap();
    let buf = term.backend().buffer();
    let text: String = (0..44)
        .map(|y| {
            (0..120)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                + "\n"
        })
        .collect();
    for needle in [
        "▶ DECK A",
        "DECK B",
        "PHASE",
        "MIXER",
        "BROWSER",
        "keyboard: kitty protocol",
        "Loaded Some Track on deck A",
    ] {
        assert!(text.contains(needle), "missing {needle}\n{text}");
    }
    // Headphone cue lit on B only.
    let cue_row = text.lines().find(|l| l.contains("CUE")).expect("cue row");
    let (a, b) = (cue_row.find('○').expect("A unlit"), cue_row.find('●').expect("B lit"));
    assert!(a < b, "{cue_row}");
}

#[test]
fn long_status_does_not_hide_the_message() {
    let view = ScreenView {
        decks: [deck(DeckId::A, true), deck(DeckId::B, false)],
        mixer: MixerView {
            crossfader: 0.0,
            faders: [1.0, 1.0],
            headphone_cue: [false, false],
            ..Default::default()
        },
        status: "x".repeat(300),
        message: "Could not load a.flac".into(),
    };
    let mut term = Terminal::new(TestBackend::new(100, 40)).unwrap();
    term.draw(|f| render_screen(f, &view)).unwrap();
    let buf = term.backend().buffer();
    let text: String = (0..40)
        .map(|y| {
            (0..100)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                + "\n"
        })
        .collect();
    assert!(text.contains("Could not load a.flac"), "{text}");
}

fn mixer_text(m: MixerView, w: u16, h: u16) -> String {
    let view = ScreenView {
        decks: [deck(DeckId::A, true), deck(DeckId::B, false)],
        mixer: m,
        status: String::new(),
        message: String::new(),
    };
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render_screen(f, &view)).unwrap();
    let buf = term.backend().buffer();
    (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
}

#[test]
fn mixer_strip_shows_trim_eq_filter_and_meters() {
    let text = mixer_text(MixerView::default(), 120, 44);
    for label in ["TRIM", "HI", "MID", "LOW", "FLT", "PK", "VOL", "CUE"] {
        assert!(text.contains(label), "missing {label}\n{text}");
    }
}

#[test]
fn killed_bands_say_kill() {
    let mut m = MixerView::default();
    m.strips[1].kills = [false, false, true];
    let text = mixer_text(m, 120, 44);
    assert!(text.contains("KILL"), "{text}");
}

#[test]
fn meters_fill_with_level() {
    let lit = |level: f32| {
        let mut m = MixerView::default();
        m.strips[0].meter = level;
        let text = mixer_text(m, 120, 44);
        let row = text.lines().find(|l| l.contains("PK")).unwrap().to_string();
        row.matches('▮').count()
    };
    assert_eq!(lit(0.0), 0);
    assert!(lit(0.1) > 0 && lit(0.1) < lit(1.0), "{} {}", lit(0.1), lit(1.0));
}

#[test]
fn narrow_mixer_bar_still_shows_volume_and_crossfader() {
    let text = mixer_text(MixerView::default(), 100, 44);
    assert!(text.contains("VOL") && text.contains("╋"), "{text}");
}
