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
    }
}

fn render(view: &DeckView, w: u16, h: u16) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| f.render_widget(DeckPanel::new(view), f.area()))
        .unwrap();
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
        ..loaded_view(false)
    };
    let text = buffer_text(&render(&view, 80, DECK_HEIGHT));
    assert!(text.contains("No track loaded"), "{text}");
}

#[test]
fn played_portion_is_dimmed_and_playhead_matches_bar_width() {
    let buf = render(&loaded_view(true), 80, DECK_HEIGHT);
    // Waveform occupies inner width 78 starting at x=1, first waveform row y=2.
    // 5 of 30 seconds played: playhead at column 1 + 78*5/30 = 14.
    let y = 2;
    assert!(buf[(3, y)].modifier.contains(Modifier::DIM));
    assert_eq!(buf[(14, y)].symbol(), "▌");
    assert!(!buf[(14, y)].modifier.contains(Modifier::DIM));
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

#[test]
fn glyph_waveform_uses_mode_colours() {
    use ratatui::style::Color;
    let mut view = loaded_view(true);
    view.waveform = vec![[-0.9, 0.9]; 200];
    view.bands = vec![[0.9, 0.0, 0.0]; 200];
    view.waveform_mode = tui::pixel::WaveformMode::Rgb;
    view.position_secs = 0.0;
    let buf = render(&view, 80, 13);
    // Centre row of the waveform, away from the playhead.
    let cell = &buf[(40, 2 + tui::WAVEFORM_ROWS / 2 - 1)];
    match cell.fg {
        Color::Rgb(r, g, b) => assert!(r >= 200 && g <= 40 && b <= 40, "{:?}", cell.fg),
        other => panic!("expected an RGB colour, got {other:?}"),
    }
}

#[test]
fn a_trace_of_treble_does_not_paint_a_vocal_white_in_3_band() {
    use ratatui::style::Color;
    let pal = tui::pixel::Palette::default();
    let mut view = loaded_view(true);
    view.waveform = vec![[-0.8, 0.8]; 200];
    view.bands = vec![[0.02, 0.8, 0.03]; 200];
    view.waveform_mode = tui::pixel::WaveformMode::ThreeBand;
    view.position_secs = 0.0;
    let buf = render(&view, 80, 13);
    let cell = &buf[(40, 2 + tui::WAVEFORM_ROWS / 2)];
    let [r, g, b, _] = pal.three_band[1].0;
    assert_eq!(
        cell.fg,
        Color::Rgb(r, g, b),
        "centre of a vocal should be amber"
    );
}

// --- CDJ-style overlays (spec 3.2) ---

fn with_cues() -> DeckView {
    let mut v = loaded_view(false);
    v.position_secs = 3.0;
    v.duration_secs = 30.0;
    v.cue_secs = Some(0.0);
    v.hot_cue_secs = [Some(15.0), None, Some(27.0), None, None, None, None, None];
    v
}

#[test]
fn hot_cues_are_marked_under_the_waveform_in_their_colours() {
    use ratatui::style::Color;
    let buf = render(&with_cues(), 80, 13);
    let marker_y = 2 + tui::WAVEFORM_ROWS;
    // Inner width 78; 15 of 30 s is column 1 + 39.
    let cell = &buf[(40, marker_y)];
    assert_eq!(cell.symbol(), "▲");
    let [r, g, b] = tui::HOT_CUE_COLOURS[0];
    assert_eq!(cell.fg, Color::Rgb(r, g, b));
    assert_eq!(buf[(41, marker_y)].symbol(), "1");
    let [r, g, b] = tui::HOT_CUE_COLOURS[2];
    assert_eq!(buf[(1 + 70, marker_y)].fg, Color::Rgb(r, g, b));
}

#[test]
fn main_cue_is_an_orange_marker() {
    use ratatui::style::Color;
    let buf = render(&with_cues(), 80, 13);
    let cell = &buf[(1, 2 + tui::WAVEFORM_ROWS)];
    assert_eq!(cell.symbol(), "▲");
    let [r, g, b] = tui::MAIN_CUE_COLOUR;
    assert_eq!(cell.fg, Color::Rgb(r, g, b));
}

#[test]
fn status_row_counts_bars_and_beats() {
    let mut v = with_cues();
    v.beat = Some((17, 3));
    let text = buffer_text(&render(&v, 80, 13));
    assert!(text.contains("BAR 17.3"), "{text}");
}

#[test]
fn end_warning_turns_the_unplayed_waveform_red() {
    use ratatui::style::Color;
    let mut v = loaded_view(true);
    v.waveform = vec![[-0.9, 0.9]; 200];
    v.position_secs = 25.0;
    v.end_warning = true;
    let buf = render(&v, 80, 13);
    let cell = &buf[(75, 2 + tui::WAVEFORM_ROWS / 2)];
    let [r, g, b] = tui::END_WARNING_COLOUR;
    assert_eq!(cell.fg, Color::Rgb(r, g, b));
}

#[test]
fn a_loop_is_bracketed_under_the_waveform_and_tinted_behind_it() {
    use ratatui::style::Color;
    let mut v = loaded_view(true);
    // Inner width 78 over 30 s: 6 s is column 1 + 15, 12 s is column 1 + 31.
    v.loop_secs = Some((6.0, 12.0));
    let buf = render(&v, 80, 13);
    let marker_y = 2 + tui::WAVEFORM_ROWS;
    let [r, g, b] = tui::LOOP_COLOUR;
    assert_eq!(buf[(16, marker_y)].symbol(), "⟦");
    assert_eq!(buf[(16, marker_y)].fg, Color::Rgb(r, g, b));
    assert_eq!(buf[(32, marker_y)].symbol(), "⟧");
    assert_eq!(buf[(32, marker_y)].fg, Color::Rgb(r, g, b));

    let inside = &buf[(24, 2 + tui::WAVEFORM_ROWS / 2)];
    assert_eq!(inside.bg, Color::Rgb(18, 46, 30), "the loop region is lit");
    let outside = &buf[(60, 2 + tui::WAVEFORM_ROWS / 2)];
    assert_ne!(outside.bg, inside.bg, "only the loop region is lit");
}

#[test]
fn the_status_row_says_when_key_lock_is_on() {
    let mut v = loaded_view(true);
    v.key_lock = true;
    assert!(buffer_text(&render(&v, 80, 13)).contains("KEY"));
    let quiet = buffer_text(&render(&loaded_view(true), 80, 13));
    assert!(!quiet.contains("KEY"), "{quiet}");
}

#[test]
fn the_status_row_shows_the_loop_and_quantize_state() {
    let mut v = loaded_view(true);
    v.loop_secs = Some((6.0, 12.0));
    v.quantize = true;
    let text = buffer_text(&render(&v, 80, 13));
    assert!(text.contains("LOOP"), "{text}");
    assert!(text.contains("QUANT"), "{text}");

    let quiet = buffer_text(&render(&loaded_view(true), 80, 13));
    assert!(!quiet.contains("LOOP"), "{quiet}");
    assert!(!quiet.contains("QUANT"), "{quiet}");
}

#[test]
fn a_loop_in_point_waiting_for_its_out_point_shows_dimmed() {
    use ratatui::style::Color;
    let mut v = loaded_view(true);
    v.loop_in_secs = Some(6.0);
    let buf = render(&v, 80, 13);
    let cell = &buf[(16, 2 + tui::WAVEFORM_ROWS)];
    let [r, g, b] = tui::LOOP_COLOUR;
    assert_eq!(cell.symbol(), "⟦");
    assert_eq!(cell.fg, Color::Rgb(r, g, b));
    assert!(
        cell.modifier.contains(Modifier::DIM),
        "not a closed loop yet"
    );

    // Once the loop closes, the pair of brackets replaces the lone one.
    v.loop_secs = Some((6.0, 12.0));
    let buf = render(&v, 80, 13);
    assert!(!buf[(16, 2 + tui::WAVEFORM_ROWS)]
        .modifier
        .contains(Modifier::DIM));
}

#[test]
fn the_unfocused_deck_is_greyed_out_so_focus_is_obvious_at_a_glance() {
    let focused = render(&loaded_view(true), 80, DECK_HEIGHT);
    let unfocused = render(&loaded_view(false), 80, DECK_HEIGHT);

    // The border, the title and the status row all go dim, not just the border style.
    for (x, y) in [(0u16, 0u16), (3, 1), (3, DECK_HEIGHT - 1)] {
        assert!(
            unfocused[(x, y)].modifier.contains(Modifier::DIM),
            "unfocused cell at {x},{y} should be dim: {:?}",
            unfocused[(x, y)].symbol()
        );
        assert!(
            !focused[(x, y)].modifier.contains(Modifier::DIM),
            "focused cell at {x},{y} should not be dim"
        );
    }
}
