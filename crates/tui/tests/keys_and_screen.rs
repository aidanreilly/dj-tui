use engine::DeckId;
use input::{Key, KeyEvent};
use ratatui::crossterm::event::{
    KeyCode, KeyEvent as CtKey, KeyEventKind, KeyEventState, KeyModifiers,
};
use ratatui::{backend::TestBackend, buffer::Buffer, style::Modifier, Terminal};
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
fn auto_repeat_arrives_as_a_press_so_holding_a_key_keeps_stepping() {
    // Terminals that report event types send Repeat while a key is held. Dropping it meant
    // holding an arrow moved the fader exactly one step on the terminals dj-tui recommends.
    let e = convert_key(ct(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Repeat));
    assert_eq!(e, Some(KeyEvent::press(Key::Down)));
    // Cue is held rather than stepped, and the keymap is what keeps it from retriggering.
    let c = convert_key(ct(
        KeyCode::Char('c'),
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ));
    assert_eq!(c, Some(KeyEvent::press(Key::Char('c'))));
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
        bands: vec![],
        waveform_mode: Default::default(),
        beat: None,
        cue_secs: None,
        hot_cue_secs: [None; 8],
        loop_secs: None,
        loop_in_secs: None,
        quantize: false,
        key_lock: false,
        end_warning: false,
        kills: [false; 3],
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
        phase: None,
        help: false,
        browser: Default::default(),
        devices: None,
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
    let (a, b) = (
        cue_row.find('○').expect("A unlit"),
        cue_row.find('●').expect("B lit"),
    );
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
        phase: None,
        help: false,
        browser: Default::default(),
        devices: None,
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
        phase: None,
        help: false,
        browser: Default::default(),
        devices: None,
    };
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render_screen(f, &view)).unwrap();
    let buf = term.backend().buffer();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                + "\n"
        })
        .collect()
}

#[test]
fn mixer_strip_shows_trim_eq_meters_and_the_master_filter() {
    let text = mixer_text(MixerView::default(), 120, 44);
    for label in ["TRIM", "HI", "MID", "LOW", "FLT", "PK", "VOL", "CUE"] {
        assert!(text.contains(label), "missing {label}\n{text}");
    }
}

#[test]
fn the_mixer_shows_the_headphone_mix() {
    let m = MixerView {
        cue_mix: 1.0,
        ..Default::default()
    };
    let text = mixer_text(m, 120, 44);
    // "MIXER" is the panel title, so look for the row label instead.
    let row = text
        .lines()
        .find(|l| l.contains("MIX ") && !l.contains("MIXER"))
        .expect("a headphone mix row");
    assert!(
        row.contains(tui::mixer::RUN),
        "all the way to master: {row}"
    );

    let m = MixerView {
        cue_mix: 0.0,
        ..Default::default()
    };
    let text = mixer_text(m, 120, 44);
    let row = text
        .lines()
        .find(|l| l.contains("MIX ") && !l.contains("MIXER"))
        .unwrap();
    assert!(
        !row.contains(tui::mixer::CAP),
        "all the way to the cue bus: {row}"
    );
}

#[test]
fn the_collapsed_mixer_lines_its_faders_up_with_its_meters() {
    // Under twelve rows the mixer folds into a bar under the decks. The fader and the meter
    // for a channel are the same channel, so they start in the same column.
    let text = mixer_text(MixerView::default(), 100, 44);
    let vol = text.lines().find(|l| l.contains("VOL")).expect("a VOL row");
    let pk = text.lines().find(|l| l.contains("PK")).expect("a PK row");
    // The column each bar starts in, counted across the whole row, since the labels are
    // different lengths and it is the bars that have to line up.
    let column = |row: &str, bar: &[char]| {
        row.chars()
            .position(|c| bar.contains(&c))
            .expect("a bar on the row")
    };
    assert_eq!(
        column(vol, &[tui::mixer::RUN, tui::mixer::TICK]),
        column(pk, &[tui::mixer::LIT_PIP, tui::mixer::UNLIT_PIP]),
        "the fader and the meter start in different columns\n{vol}\n{pk}"
    );
}

#[test]
fn the_collapsed_faders_are_a_marker_on_a_rail() {
    // A fader's value is where it sits, which is what the crossfader under it already says.
    let text = mixer_text(MixerView::default(), 100, 44);
    let vol = text.lines().find(|l| l.contains("VOL")).expect("a VOL row");
    assert!(vol.contains(tui::mixer::TICK), "no marker: {vol}");
    assert!(!vol.contains(tui::mixer::CAP), "nothing is filled: {vol}");
}

/// The foreground of the first cell on `label`'s row that holds one of `glyphs`.
fn colour_of(buf: &Buffer, label: &str, glyphs: &[char]) -> ratatui::style::Color {
    for y in 0..buf.area.height {
        let row: String = (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect();
        if !row.contains(label) {
            continue;
        }
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            if cell
                .symbol()
                .chars()
                .next()
                .is_some_and(|c| glyphs.contains(&c))
            {
                return cell.fg;
            }
        }
    }
    panic!("no {glyphs:?} on a row with {label}");
}

#[test]
fn a_killed_band_is_muted_rather_than_shouted() {
    // KILL is the state of a band, not a value being read off the row, so it recedes with
    // the rail instead of being the brightest thing in the strip.
    let mut m = MixerView::default();
    m.strips[0].kills = [false, false, true];
    let buf = mixer_buffer(m, 120, 44);
    let [r, g, b] = tui::theme::BASE01;
    assert_eq!(
        // 'K' alone: the row's own label is "HI", whose I would be found first.
        colour_of(&buf, "KILL", &['K']),
        ratatui::style::Color::Rgb(r, g, b)
    );
}

#[test]
fn a_faders_marker_is_brighter_than_the_rail_it_sits_on() {
    // The collapsed mixer draws a fader as a marker on a rail. Colouring by glyph put the
    // rail in the foreground and the marker in the muted grey, which is backwards: the
    // marker is the value.
    let buf = mixer_buffer(MixerView::default(), 100, 44);
    let [r, g, b] = tui::theme::BASE01;
    let muted = ratatui::style::Color::Rgb(r, g, b);
    assert_eq!(
        colour_of(&buf, "VOL", &[tui::mixer::RUN]),
        muted,
        "the rail recedes"
    );
    assert_ne!(
        colour_of(&buf, "VOL", &[tui::mixer::TICK]),
        muted,
        "the marker does not"
    );
}

#[test]
fn the_fade_length_rides_the_crossfader_row() {
    // It belongs with the crossfader: both are about the move between the decks. A row of
    // its own put it in the label gutter with the per-channel controls, where it is not one.
    let text = mixer_text(MixerView::default(), 120, 44);
    let row = text
        .lines()
        .find(|l| l.contains("FADE"))
        .expect("a row with the fade length");
    assert!(
        row.contains("A \u{2501}"),
        "it rides the crossfader row: {row}"
    );
    // The length is right-aligned to the border, so it stays put as the number changes
    // width. What it must never do is run into the crossfader's B, which is what measuring
    // the room by the control rows rather than by the panel produced.
    // The b is the unit: a fade is a count of beats, not seconds.
    assert!(row.contains("FADE 8b"), "{row}");
    assert!(!row.contains("BFADE"), "it ran into the crossfader: {row}");
    assert!(
        !text.contains("fade"),
        "the old lowercase label is gone: {text}"
    );
}

#[test]
fn the_longest_fade_length_still_fits_the_panel() {
    // The crossfader row is the widest thing in the strip, and 64 beats is the longest fade
    // the keys reach. A two-digit count used to run past the right border.
    let m = MixerView {
        fade_beats: 64.0,
        ..Default::default()
    };
    let buf = mixer_buffer(m, 120, 44);
    let row = (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .find(|l| l.contains("FADE"))
        .expect("a row with the fade length");
    assert!(row.contains("FADE 64b"), "the count was cut off: {row}");
    assert!(
        !row.contains("BFADE"),
        "the longest count still needs its gap: {row}"
    );
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
    // Lit and unlit are both squares, so counting one of them counts the level.
    let lit = |level: f32| {
        let mut m = MixerView::default();
        m.strips[0].meter = level;
        let text = mixer_text(m, 120, 44);
        let row = text.lines().find(|l| l.contains("PK")).unwrap().to_string();
        row.matches('■').count()
    };
    assert_eq!(lit(0.0), 0);
    assert!(
        lit(0.1) > 0 && lit(0.1) < lit(1.0),
        "{} {}",
        lit(0.1),
        lit(1.0)
    );
}

#[test]
fn narrow_mixer_bar_still_shows_volume_and_crossfader() {
    let text = mixer_text(MixerView::default(), 100, 44);
    assert!(text.contains("VOL") && text.contains("╋"), "{text}");
}

#[test]
fn phase_meter_shows_the_offset_between_decks() {
    let render_phase = |phase: Option<f64>| {
        let view = ScreenView {
            decks: [deck(DeckId::A, true), deck(DeckId::B, false)],
            mixer: MixerView::default(),
            status: String::new(),
            message: String::new(),
            phase,
            help: false,
            browser: Default::default(),
            devices: None,
        };
        let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
        term.draw(|f| render_screen(f, &view)).unwrap();
        let l = tui::screen_layout(ratatui::layout::Rect::new(0, 0, 120, 44));
        let buf = term.backend().buffer();
        (l.phase.x..l.phase.right())
            .map(|x| buf[(x, l.phase.y)].symbol().to_string())
            .collect::<String>()
    };
    let none = render_phase(None);
    assert!(
        none.contains("PHASE") && none.contains("both decks running"),
        "{none}"
    );
    let centred = render_phase(Some(0.0));
    let ahead = render_phase(Some(0.25));
    let pos = |s: &str| s.chars().position(|c| c == '█').expect("marker");
    assert!(pos(&ahead) > pos(&centred), "{centred}\n{ahead}");
}

#[test]
fn the_help_overlay_lists_the_keys_over_the_screen() {
    let mut view = ScreenView {
        decks: [deck(DeckId::A, true), deck(DeckId::B, false)],
        mixer: MixerView::default(),
        status: String::new(),
        message: String::new(),
        phase: None,
        help: false,
        browser: Default::default(),
        devices: None,
    };
    let text = |view: &ScreenView| {
        let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
        term.draw(|f| render_screen(f, view)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    + "\n"
            })
            .collect::<String>()
    };
    assert!(!text(&view).contains("HELP"), "hidden until asked for");

    view.help = true;
    let shown = text(&view);
    assert!(shown.contains("HELP"), "{shown}");
    for label in ["Space", "Tab", "hot cue", "loop", "Ctrl+Q"] {
        assert!(shown.contains(label), "missing {label}\n{shown}");
    }
}

#[test]
fn the_help_overlay_fits_a_small_terminal() {
    let view = ScreenView {
        decks: [deck(DeckId::A, true), deck(DeckId::B, false)],
        mixer: MixerView::default(),
        status: String::new(),
        message: String::new(),
        phase: None,
        help: true,
        browser: Default::default(),
        devices: None,
    };
    // Nothing here should panic or write outside the buffer.
    for (w, h) in [(40, 12), (60, 20), (200, 60)] {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render_screen(f, &view)).unwrap();
    }
}

fn mixer_buffer(m: MixerView, w: u16, h: u16) -> Buffer {
    let view = ScreenView {
        decks: [deck(DeckId::A, true), deck(DeckId::B, false)],
        mixer: m,
        status: String::new(),
        message: String::new(),
        phase: None,
        help: false,
        browser: Default::default(),
        devices: None,
    };
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render_screen(f, &view)).unwrap();
    term.backend().buffer().clone()
}

/// The VOL row's y, and the x of deck A's and deck B's bars on it. The row is
/// `{label:<5}{a} {b}` inside a bordered block, so A starts at 1+5 and B a bar and a space on.
fn vol_row_columns(buf: &Buffer) -> (u16, u16, u16) {
    let y = (0..buf.area.height)
        .find(|&y| {
            (0..buf.area.width - 4).any(|x| {
                (0..4).all(|i| buf[(x + i, y)].symbol() == ["V", "O", "L", " "][i as usize])
            })
        })
        .expect("a VOL row");
    let a_x = (0..buf.area.width - 4)
        .find(|&x| (0..3).all(|i| buf[(x + i, y)].symbol() == ["V", "O", "L"][i as usize]))
        .expect("the VOL label");
    (y, a_x + 5, a_x + 5 + 8)
}

#[test]
fn the_unfocused_channel_is_greyed_out_in_the_mixer() {
    let a_focused = mixer_buffer(
        MixerView {
            focused: DeckId::A,
            ..Default::default()
        },
        120,
        44,
    );
    let b_focused = mixer_buffer(
        MixerView {
            focused: DeckId::B,
            ..Default::default()
        },
        120,
        44,
    );

    let (y, ax, bx) = vol_row_columns(&a_focused);
    assert!(
        a_focused[(bx, y)].modifier.contains(Modifier::DIM),
        "B is dim while A has focus"
    );
    assert!(
        !a_focused[(ax, y)].modifier.contains(Modifier::DIM),
        "A is not"
    );
    assert!(
        b_focused[(ax, y)].modifier.contains(Modifier::DIM),
        "A is dim while B has focus"
    );
    assert!(
        !b_focused[(bx, y)].modifier.contains(Modifier::DIM),
        "B is not"
    );
}

/// The deck panel's top border row, where the title and the band markers live.
fn deck_title_row(kills: [bool; 3]) -> (String, Buffer) {
    let mut a = deck(DeckId::A, true);
    a.kills = kills;
    let view = ScreenView {
        decks: [a, deck(DeckId::B, false)],
        mixer: MixerView::default(),
        status: String::new(),
        message: String::new(),
        phase: None,
        help: false,
        browser: Default::default(),
        devices: None,
    };
    let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
    term.draw(|f| render_screen(f, &view)).unwrap();
    let buf = term.backend().buffer().clone();
    let (w, h) = (buf.area.width, buf.area.height);
    let row = (0..h)
        .find(|&y| {
            (0..w).any(|x| buf[(x, y)].symbol() == "\u{2503}" || buf[(x, y)].symbol() == "\u{250f}")
        })
        .expect("a deck panel border row");
    let text: String = (0..w).map(|x| buf[(x, row)].symbol().to_string()).collect();
    (text, buf)
}

#[test]
fn the_deck_title_names_the_bands() {
    let (row, _) = deck_title_row([false; 3]);
    assert!(row.contains("[low] [mid] [hi]"), "{row}");
}

#[test]
fn a_killed_band_is_dimmed_in_the_deck_title() {
    let (row, buf) = deck_title_row([false, true, false]);
    let y = (0..buf.area.height)
        .find(|&y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                == row
        })
        .unwrap();
    // `row` concatenates one glyph per column, but a glyph like the thick border corners
    // is multiple bytes, so a substring's byte offset is not its column: count chars instead.
    let hi = row[..row.find("[hi]").expect("{row}")].chars().count() as u16;
    let mid = row[..row.find("[mid]").expect("{row}")].chars().count() as u16;
    assert!(
        buf[(mid, y)].modifier.contains(Modifier::DIM),
        "a killed band is dimmed"
    );
    assert!(
        !buf[(hi, y)].modifier.contains(Modifier::DIM),
        "a live band is not"
    );
}

/// Both decks with a bpm and key set, rendered at a chosen (sub-wide-layout) terminal width
/// so the deck panel gets exactly that width. Returns deck A's and deck B's title rows.
fn deck_rows_with_bpm_and_key(width: u16) -> (String, String) {
    let mut a = deck(DeckId::A, true);
    a.bpm = Some(124.0);
    a.key = Some("8A".into());
    let mut b = deck(DeckId::B, false);
    b.bpm = Some(124.0);
    b.key = Some("8A".into());
    let view = ScreenView {
        decks: [a, b],
        mixer: MixerView::default(),
        status: String::new(),
        message: String::new(),
        phase: None,
        help: false,
        browser: Default::default(),
        devices: None,
    };
    let mut term = Terminal::new(TestBackend::new(width, 44)).unwrap();
    term.draw(|f| render_screen(f, &view)).unwrap();
    let buf = term.backend().buffer().clone();
    let text: String = (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                + "\n"
        })
        .collect();
    let a_row = text
        .lines()
        .find(|l| l.contains("DECK A"))
        .expect("deck A's row")
        .to_string();
    let b_row = text
        .lines()
        .find(|l| l.contains("DECK B"))
        .expect("deck B's row")
        .to_string();
    (a_row, b_row)
}

#[test]
fn a_narrow_panel_drops_the_markers_but_keeps_bpm_and_key() {
    // Below either deck's threshold: there is no shorter marker form, so both are dropped
    // and the BPM/key — what a glance needs most — stay.
    let (a_row, b_row) = deck_rows_with_bpm_and_key(38);
    for (name, row) in [("A", &a_row), ("B", &b_row)] {
        assert!(!row.contains("[hi]"), "deck {name} kept a marker: {row}");
        assert!(!row.contains("[mid]"), "deck {name} kept a marker: {row}");
        assert!(!row.contains("[low]"), "deck {name} kept a marker: {row}");
        assert!(row.contains("124.00"), "deck {name} lost its bpm: {row}");
        assert!(row.contains("8A"), "deck {name} lost its key: {row}");
    }
}

#[test]
fn the_focused_and_unfocused_decks_agree_on_whether_the_markers_fit() {
    // Deck A's focused label carries `▶`, three bytes for one column. Sized to sit exactly
    // where a byte count (rather than a column count) used to make deck A alone think the
    // markers didn't fit, while deck B — plain ASCII, unaffected either way — already showed
    // them: both panels are always the same width in the real layout, so that disagreement
    // was the bug (finding 1). Fixed, both read the same width the same way and agree.
    let (a_row, b_row) = deck_rows_with_bpm_and_key(41);
    assert!(
        a_row.contains("[hi]"),
        "deck A dropped the markers: {a_row}"
    );
    assert!(
        b_row.contains("[hi]"),
        "deck B dropped the markers: {b_row}"
    );
}
