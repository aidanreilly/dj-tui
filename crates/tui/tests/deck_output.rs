//! The mixer panel in deck output mode: meters and a label, nothing that lies.

use engine::{DeckId, OutputMode};
use ratatui::{backend::TestBackend, Terminal};
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
