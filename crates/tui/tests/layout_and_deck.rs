//! Screen layout (spec 2) and deck panel rendering.

use engine::DeckId;
use ratatui::{backend::TestBackend, buffer::Buffer, layout::Rect, style::Modifier, Terminal};
use tui::{screen_layout, DeckPanel, DeckView, DECK_HEIGHT, WAVEFORM_ROWS};

fn buffer_text(buf: &Buffer) -> String {
    let area = buf.area;
    let mut s = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            s.push_str(buf[(x, y)].symbol());
        }
        s.push('\n');
    }
    s
}

fn loaded_view(focused: bool) -> DeckView {
    DeckView {
        id: DeckId::A,
        focused,
        title: Some("Artist - Title".into()),
        bpm: Some(124.0),
        key: Some("8A".into()),
        position_secs: 5.0,
        duration_secs: 30.0,
        loading: false,
        playing: true,
        hot_cues: [true, false, true, false, false, false, false, false],
        waveform: vec![[-1.0, 1.0]; 200],
    }
}

fn render(view: &DeckView, w: u16, h: u16) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| f.render_widget(DeckPanel::new(view), f.area())).unwrap();
    term.backend().buffer().clone()
}

#[test]
fn wide_terminal_stacks_decks_with_mixer_on_the_right() {
    let l = screen_layout(Rect::new(0, 0, 120, 44));
    assert_eq!(l.deck_a.x, l.deck_b.x);
    assert!(l.deck_a.y < l.phase.y && l.phase.y < l.deck_b.y);
    assert_eq!(l.deck_a.width, l.deck_b.width);
    assert!(l.mixer.x >= l.deck_a.right());
    assert!(l.browser.y >= l.deck_b.bottom());
    assert_eq!(l.browser.width, 120);
}

#[test]
fn narrow_terminal_moves_mixer_under_the_decks() {
    let l = screen_layout(Rect::new(0, 0, 100, 44));
    assert_eq!(l.deck_a.width, 100);
    assert_eq!(l.mixer.width, 100);
    assert!(l.mixer.y >= l.deck_b.bottom());
    assert!(l.browser.y >= l.mixer.bottom());
}

#[test]
fn focused_deck_has_heavy_border_and_marker() {
    let buf = render(&loaded_view(true), 80, DECK_HEIGHT);
    let text = buffer_text(&buf);
    assert!(text.contains("▶ DECK A"), "{text}");
    assert_eq!(buf[(0, 0)].symbol(), "┏");
}

#[test]
fn unfocused_deck_has_plain_border() {
    let buf = render(&loaded_view(false), 80, DECK_HEIGHT);
    let text = buffer_text(&buf);
    assert!(text.contains("DECK A") && !text.contains("▶"));
    assert_eq!(buf[(0, 0)].symbol(), "┌");
}

#[test]
fn header_shows_bpm_key_and_times() {
    let text = buffer_text(&render(&loaded_view(true), 80, DECK_HEIGHT));
    assert!(text.contains("124.00"), "{text}");
    assert!(text.contains("8A"));
    assert!(text.contains("Artist - Title"));
    assert!(text.contains("00:05"));
    assert!(text.contains("-00:25"));
}

#[test]
fn hot_cue_row_marks_set_cues() {
    let text = buffer_text(&render(&loaded_view(true), 80, DECK_HEIGHT));
    assert!(text.contains("[1][ ][3][ ][ ][ ][ ][ ]"), "{text}");
}

#[test]
fn empty_deck_says_so() {
    let view = DeckView {
        title: None,
        waveform: vec![],
        ..loaded_view(false)
    };
    let text = buffer_text(&render(&view, 80, DECK_HEIGHT));
    assert!(text.contains("No track loaded"), "{text}");
}

#[test]
fn played_portion_is_dimmed_and_playhead_is_reversed() {
    let buf = render(&loaded_view(true), 80, DECK_HEIGHT);
    // Waveform occupies inner width 78 starting at x=1, first waveform row y=2.
    // 5 of 30 seconds played: playhead at column 1 + 78*5/30 = 14.
    let y = 2;
    assert!(buf[(3, y)].modifier.contains(Modifier::DIM));
    assert!(buf[(14, y)].modifier.contains(Modifier::REVERSED));
    assert!(!buf[(40, y)].modifier.contains(Modifier::DIM));
}

#[test]
fn the_waveform_gets_eight_rows_inside_the_deck_panel() {
    // Borders, title, waveform, marker row and status row.
    assert_eq!(WAVEFORM_ROWS, 8);
    assert_eq!(DECK_HEIGHT, 2 + 1 + WAVEFORM_ROWS + 1 + 1);
}

#[test]
fn every_waveform_row_is_drawn() {
    let buf = render(&loaded_view(true), 80, DECK_HEIGHT);
    for y in 2..(2 + WAVEFORM_ROWS) {
        assert_eq!(buf[(3, y)].symbol(), "▌", "row {y} empty");
    }
}
