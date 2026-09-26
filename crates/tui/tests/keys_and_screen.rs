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
        },
        status: "keyboard: kitty protocol".into(),
        message: "Loaded Some Track on deck A".into(),
        waveform_mode: wave::WaveformMode::default(),
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
    assert!(text.contains("CUE ○") && text.contains("● "), "{text}");
}

#[test]
fn long_status_does_not_hide_the_message() {
    let view = ScreenView {
        decks: [deck(DeckId::A, true), deck(DeckId::B, false)],
        mixer: MixerView {
            crossfader: 0.0,
            faders: [1.0, 1.0],
            headphone_cue: [false, false],
        },
        status: "x".repeat(300),
        message: "Could not load a.flac".into(),
        waveform_mode: wave::WaveformMode::default(),
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

#[test]
fn the_waveform_mode_reaches_the_glyph_renderer() {
    use ratatui::style::Color;
    use wave::{WavePoint, WaveformMode};

    // A loaded deck whose bass dominates: 3band paints it blue, rgb paints it red.
    fn view(mode: WaveformMode) -> ScreenView {
        let mut d = deck(DeckId::A, true);
        d.title = Some("Track".into());
        d.duration_secs = 100.0;
        d.waveform = vec![
            WavePoint {
                range: [-1.0, 1.0],
                bands: [1.0, 0.0, 0.0],
            };
            200
        ];
        ScreenView {
            decks: [d, deck(DeckId::B, false)],
            mixer: MixerView {
                crossfader: 0.0,
                faders: [1.0, 1.0],
                headphone_cue: [false, false],
            },
            status: String::new(),
            message: String::new(),
            waveform_mode: mode,
        }
    }

    fn waveform_colours(mode: WaveformMode) -> Vec<Color> {
        let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
        let v = view(mode);
        term.draw(|f| render_screen(f, &v)).unwrap();
        let buf = term.backend().buffer();
        (0..44)
            .flat_map(|y| (0..120).map(move |x| (x, y)))
            .filter(|&(x, y)| buf[(x, y)].symbol() == "▌")
            .map(|(x, y)| buf[(x, y)].fg)
            .collect()
    }

    /// The distinct colours drawn, so a failure prints a census rather than every cell.
    fn distinct(colours: &[Color]) -> Vec<Color> {
        let mut v: Vec<Color> = Vec::new();
        for c in colours {
            if !v.contains(c) {
                v.push(*c);
            }
        }
        v
    }

    let three = waveform_colours(WaveformMode::ThreeBand);
    let rgb = waveform_colours(WaveformMode::Rgb);
    assert!(!three.is_empty(), "no waveform bars were drawn");
    assert!(
        three.contains(&Color::Rgb(36, 82, 200)),
        "3band should paint bass-heavy columns blue, saw {:?}",
        distinct(&three)
    );
    assert!(
        rgb.contains(&Color::Rgb(255, 0, 0)),
        "rgb should paint bass-heavy columns red, saw {:?}",
        distinct(&rgb)
    );
}
