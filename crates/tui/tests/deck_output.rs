//! The mixer panel in deck output mode: meters and a label, nothing that lies.

use engine::{DeckId, OutputMode};
use ratatui::{backend::TestBackend, style::Modifier, Terminal};
use tui::{render_screen, DeckView, MixerView, ScreenView};

/// An empty deck. `DeckView` has no `Default`, so this mirrors the helper in
/// `keys_and_screen.rs`.
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

fn screen(output: OutputMode) -> ScreenView {
    ScreenView {
        decks: [deck(DeckId::A), deck(DeckId::B)],
        mixer: MixerView {
            output,
            ..Default::default()
        },
        status: String::new(),
        message: String::new(),
        phase: None,
        help: false,
        browser: Default::default(),
        devices: None,
    }
}

fn render(output: OutputMode, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render_screen(f, &screen(output))).unwrap();
    let buf = term.backend().buffer();
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol().to_string())
        .collect()
}

#[test]
fn the_wide_panel_names_the_mode_and_both_output_pairs() {
    let out = render(OutputMode::Decks, 120, 44);
    assert!(out.contains("DECK OUT"), "{out}");
    assert!(out.contains("1/2"), "{out}");
    assert!(out.contains("3/4"), "{out}");
}

#[test]
fn the_wide_panel_drops_the_controls_it_cannot_drive() {
    let out = render(OutputMode::Decks, 120, 44);
    for gone in ["TRIM", "FLT", "MSTR", "MIX"] {
        assert!(
            !out.contains(gone),
            "{gone} should not be drawn in deck mode: {out}"
        );
    }
}

#[test]
fn the_compact_bar_names_the_mode() {
    // Under WIDE_MIN_WIDTH the mixer collapses to the bar under the decks.
    let out = render(OutputMode::Decks, 100, 44);
    assert!(out.contains("DECK OUT"), "{out}");
}

#[test]
fn mix_mode_still_draws_the_full_strip() {
    let out = render(OutputMode::Mix, 120, 44);
    for row in ["TRIM", "FLT", "MSTR", "MIX"] {
        assert!(out.contains(row), "{row} missing from the mix strip: {out}");
    }
    assert!(!out.contains("DECK OUT"), "{out}");
}

/// Every cell's symbol plus whether it is DIM, which is how a killed band marker is drawn.
fn styled_screen(output: OutputMode, kills: [bool; 3]) -> Vec<(String, bool)> {
    let mut view = screen(output);
    view.decks[0].kills = kills;
    view.decks[1].kills = kills;
    let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
    term.draw(|f| render_screen(f, &view)).unwrap();
    let buf = term.backend().buffer();
    (0..44)
        .flat_map(|y| (0..120).map(move |x| (x, y)))
        .map(|(x, y)| {
            (
                buf[(x, y)].symbol().to_string(),
                buf[(x, y)].modifier.contains(Modifier::DIM),
            )
        })
        .collect()
}

#[test]
fn killing_a_band_changes_nothing_on_screen_in_deck_mode() {
    // Deck mode bypasses the EQ, so a lit band marker would claim an effect the audio never
    // sees. The screen must be identical whether or not the kills are set.
    assert_eq!(
        styled_screen(OutputMode::Decks, [true; 3]),
        styled_screen(OutputMode::Decks, [false; 3]),
        "a kill must not show anywhere on screen in deck mode"
    );
}

#[test]
fn killing_a_band_still_shows_in_mix_mode() {
    assert_ne!(
        styled_screen(OutputMode::Mix, [true; 3]),
        styled_screen(OutputMode::Mix, [false; 3]),
        "mix mode must still mark a killed band"
    );
}

/// The rows of the deck-out panel, trimmed of the border, as plain strings.
fn deck_out_rows() -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
    term.draw(|f| render_screen(f, &screen(OutputMode::Decks)))
        .unwrap();
    let buf = term.backend().buffer();
    // The panel sits in the rightmost 26 columns; drop its two border columns.
    (0..44)
        .map(|y| {
            (94..119)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn each_output_label_sits_over_its_own_meter() {
    let rows = deck_out_rows();
    let pk_row = rows
        .iter()
        .position(|r| r.contains(tui::mixer::UNLIT_PIP))
        .expect("the meter row");
    // The header is the row directly above the meters it labels.
    let header = &rows[pk_row - 1];
    let pk = &rows[pk_row];

    let pips: Vec<usize> = pk
        .chars()
        .enumerate()
        .filter(|(_, c)| *c == tui::mixer::UNLIT_PIP)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(pips.len(), tui::mixer::BAR * 2, "two meters of BAR pips");
    let centre_a = pips[tui::mixer::BAR / 2];
    let centre_b = pips[tui::mixer::BAR + tui::mixer::BAR / 2];

    assert_eq!(
        header.chars().position(|c| c == 'A'),
        Some(centre_a),
        "A should sit over its meter\n{header}\n{pk}"
    );
    assert_eq!(
        header.chars().position(|c| c == 'B'),
        Some(centre_b),
        "B should sit over its meter\n{header}\n{pk}"
    );
}
