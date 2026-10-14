//! The folded EQ row and the transition list in the mixer panel.

use engine::DeckId;
use ratatui::{backend::TestBackend, buffer::Buffer, style::Color, style::Modifier, Terminal};
use tui::{
    render_screen, theme, DeckView, MixerView, ScreenView, SlotState, SlotView, TransitionView,
};

/// An empty deck, as the other screen tests build one.
fn deck(id: DeckId) -> DeckView {
    DeckView {
        id,
        focused: id == DeckId::A,
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

fn view(mixer: MixerView) -> ScreenView {
    ScreenView {
        decks: [deck(DeckId::A), deck(DeckId::B)],
        mixer,
        status: String::new(),
        message: String::new(),
        phase: None,
        help: false,
        browser: Default::default(),
        devices: None,
    }
}

fn buffer(mixer: MixerView, w: u16, h: u16) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render_screen(f, &view(mixer))).unwrap();
    term.backend().buffer().clone()
}

fn text(buf: &Buffer) -> String {
    let area = buf.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                + "\n"
        })
        .collect()
}

/// Where `needle` first appears, as (x, y) of its first cell.
fn find(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
    let t = text(buf);
    for (y, line) in t.lines().enumerate() {
        if let Some(byte) = line.find(needle) {
            let x = line[..byte].chars().count();
            return Some((x as u16, y as u16));
        }
    }
    None
}

fn rgb(c: [u8; 3]) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}

fn nine() -> Vec<SlotView> {
    [
        "Bass swap",
        "Long blend",
        "Filter blend",
        "Jump cut",
        "Drop swap",
        "Loop out",
        "Tempo bridge",
        "Brake",
        "Auto",
    ]
    .iter()
    .map(|n| SlotView {
        name: n.to_string(),
        detail: "16 bars".into(),
        state: SlotState::Ready,
    })
    .collect()
}

#[test]
fn the_eq_folds_into_one_row_that_reads_flat_when_nothing_is_moved() {
    let t = text(&buffer(MixerView::default(), 120, 44));
    assert!(t.contains("EQ "), "{t}");
    assert!(t.contains("flat"), "{t}");
    for gone in ["TRIM", "HI ", "MID ", "LOW "] {
        assert!(!t.contains(gone), "{gone} should be folded away\n{t}");
    }
}

#[test]
fn a_moved_band_lights_its_letter_in_the_colour_of_what_happened() {
    let mut m = MixerView::default();
    m.strips[0].eq_db = [-12.0, 0.0, 3.0];
    m.strips[1].kills = [false, true, false];
    let buf = buffer(m, 120, 44);
    let (x, y) = find(&buf, "LMH").expect("deck A's letters");
    assert_eq!(buf[(x, y)].fg, rgb(theme::EQ_CUT), "L is cut");
    assert_eq!(buf[(x + 1, y)].fg, rgb(theme::BASE01), "M is flat");
    assert_eq!(buf[(x + 2, y)].fg, rgb(theme::EQ_BOOST), "H is up");
    let row: String = (0..buf.area.width)
        .map(|i| buf[(i, y)].symbol().to_string())
        .collect();
    let second = row.rfind("LMH").expect("deck B's letters");
    let bx = row[..second].chars().count() as u16;
    assert_eq!(
        buf[(bx + 1, y)].fg,
        rgb(theme::EQ_KILL),
        "B's mids are killed"
    );
}

#[test]
fn a_trim_off_zero_shows_in_the_row() {
    let mut m = MixerView::default();
    m.strips[1].trim_db = 2.0;
    let t = text(&buffer(m, 120, 44));
    assert!(t.contains("+2"), "{t}");
}

#[test]
fn the_full_rails_come_back_when_asked_for() {
    let m = MixerView {
        full_eq: true,
        ..Default::default()
    };
    let t = text(&buffer(m, 120, 44));
    for label in ["TRIM", "HI", "MID", "LOW"] {
        assert!(t.contains(label), "{label}\n{t}");
    }
}

#[test]
fn all_nine_keys_are_listed_under_the_mixer_with_what_comes_next() {
    let m = MixerView {
        transitions: TransitionView {
            layer: true,
            slots: nine(),
            next: vec!["9 next: Long blend".into(), "A→B out phrase".into()],
        },
        ..Default::default()
    };
    let t = text(&buffer(m, 120, 44));
    for (name, n) in ["Bass swap", "Filter blend", "Tempo bridge", "Auto"]
        .iter()
        .zip([1, 3, 7, 9])
    {
        assert!(t.contains(&format!("{n} {name}")), "{n} {name}\n{t}");
    }
    assert!(t.contains("16 bars"), "{t}");
    assert!(t.contains("9 next: Long blend"), "{t}");
    assert!(t.contains("1-9"), "the header says the keys are live\n{t}");
}

#[test]
fn outside_the_layer_the_header_says_how_to_get_in() {
    let m = MixerView {
        transitions: TransitionView {
            layer: false,
            slots: nine(),
            next: Vec::new(),
        },
        ..Default::default()
    };
    let t = text(&buffer(m, 120, 44));
    assert!(t.contains("n for keys"), "{t}");
}

#[test]
fn the_armed_key_stands_out_and_a_switched_off_one_is_struck_through() {
    let mut slots = nine();
    slots[0].state = SlotState::Armed;
    slots[0].detail = "▸ 6 bars".into();
    slots[1].state = SlotState::Disabled;
    slots[1].detail = "off".into();
    slots[2].state = SlotState::Running;
    let m = MixerView {
        transitions: TransitionView {
            layer: true,
            slots,
            next: Vec::new(),
        },
        ..Default::default()
    };
    let buf = buffer(m, 120, 44);
    let (x, y) = find(&buf, "1 Bass swap").unwrap();
    assert_eq!(buf[(x + 2, y)].fg, rgb(theme::SLOT_ARMED));
    let (x, y) = find(&buf, "2 Long blend").unwrap();
    assert!(buf[(x + 2, y)].modifier.contains(Modifier::CROSSED_OUT));
    let (x, y) = find(&buf, "3 Filter blend").unwrap();
    assert_eq!(buf[(x + 2, y)].fg, rgb(theme::SLOT_RUNNING));
}

#[test]
fn the_narrow_bar_carries_the_armed_key_on_a_line_of_its_own() {
    let mut slots = nine();
    slots[3].state = SlotState::Armed;
    slots[3].detail = "▸ 2 bars".into();
    let m = MixerView {
        transitions: TransitionView {
            layer: true,
            slots,
            next: Vec::new(),
        },
        ..Default::default()
    };
    let t = text(&buffer(m, 100, 44));
    assert!(t.contains("4 Jump cut"), "{t}");
    assert!(t.contains("▸ 2 bars"), "{t}");
}
