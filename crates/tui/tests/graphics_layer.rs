//! The kitty-graphics layer drawn over the glyph waveforms, exercised through ratatui's
//! TestBackend with the kitty protocol forced on.

use engine::DeckId;
use ratatui::{backend::TestBackend, buffer::Buffer, layout::Rect, Terminal};
use ratatui_image::picker::{Picker, ProtocolType};
use tui::{
    render_screen, screen_layout, waveform_area, DeckView, Graphics, MixerView, ScreenView,
    WAVEFORM_ROWS,
};

const KITTY_PLACEHOLDER: char = '\u{10EEEE}';
const KITTY_APC: &str = "\x1b_G";

fn deck(id: DeckId, loaded: bool, position_secs: f64) -> DeckView {
    DeckView {
        id,
        focused: id == DeckId::A,
        title: loaded.then(|| "Track".to_string()),
        bpm: None,
        key: None,
        position_secs,
        duration_secs: if loaded { 180.0 } else { 0.0 },
        loading: false,
        playing: loaded,
        hot_cues: [false; 8],
        waveform: if loaded {
            (0..512)
                .map(|i| {
                    let p = 0.2 + 0.7 * ((i % 37) as f32 / 37.0);
                    [-p, p]
                })
                .collect()
        } else {
            vec![]
        },
        bands: vec![],
        waveform_mode: Default::default(),
        beat: None,
        cue_secs: None,
        hot_cue_secs: [None; 8],
        end_warning: false,
    }
}

fn view(pos_a: f64, b_loaded: bool) -> ScreenView {
    ScreenView {
        decks: [
            deck(DeckId::A, true, pos_a),
            deck(DeckId::B, b_loaded, 10.0),
        ],
        mixer: MixerView {
            crossfader: 0.0,
            faders: [1.0, 1.0],
            headphone_cue: [false, false],
            ..Default::default()
        },
        status: String::new(),
        message: String::new(),
        phase: None,
    }
}

fn kitty() -> Picker {
    let mut p = Picker::halfblocks();
    p.set_protocol_type(ProtocolType::Kitty);
    p
}

fn draw(term: &mut Terminal<TestBackend>, g: &mut Graphics, v: &ScreenView) -> Buffer {
    term.draw(|f| {
        render_screen(f, v);
        g.render(f, v);
    })
    .unwrap();
    term.backend().buffer().clone()
}

fn area_has(buf: &Buffer, area: Rect, pred: impl Fn(&str) -> bool) -> bool {
    (area.y..area.bottom()).any(|y| (area.x..area.right()).any(|x| pred(buf[(x, y)].symbol())))
}

fn anywhere_has(buf: &Buffer, needle: &str) -> bool {
    area_has(buf, buf.area, |s| s.contains(needle))
}

#[test]
fn waveform_area_is_the_rows_under_the_title() {
    let panel = Rect::new(3, 4, 80, tui::DECK_HEIGHT);
    let a = waveform_area(panel);
    assert_eq!(a, Rect::new(4, 6, 78, WAVEFORM_ROWS));
}

#[test]
fn loaded_decks_get_a_kitty_image_and_empty_decks_do_not() {
    let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
    let mut g = Graphics::new(kitty());
    let buf = draw(&mut term, &mut g, &view(30.0, false));
    let l = screen_layout(Rect::new(0, 0, 120, 44));
    let placeholder = |s: &str| s.contains(KITTY_PLACEHOLDER);
    assert!(
        area_has(&buf, waveform_area(l.deck_a), placeholder),
        "deck A has no image"
    );
    assert!(
        !area_has(&buf, waveform_area(l.deck_b), placeholder),
        "empty deck B got an image"
    );
    assert!(anywhere_has(&buf, KITTY_APC), "image data was never sent");
    assert_eq!(g.transmissions(), 1);
}

#[test]
fn unchanged_frames_send_no_image_data() {
    let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
    let mut g = Graphics::new(kitty());
    draw(&mut term, &mut g, &view(30.0, true));
    let buf = draw(&mut term, &mut g, &view(30.0, true));
    assert!(
        !anywhere_has(&buf, KITTY_APC),
        "image data resent without a change"
    );
    assert_eq!(g.transmissions(), 2, "one per deck");
}

#[test]
fn a_playhead_step_of_one_pixel_resends_only_that_deck() {
    let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
    let mut g = Graphics::new(kitty());
    draw(&mut term, &mut g, &view(30.0, true));
    // 180 s over roughly 940 px is about 0.19 s per pixel; move by two seconds.
    let buf = draw(&mut term, &mut g, &view(32.0, true));
    assert!(anywhere_has(&buf, KITTY_APC));
    assert_eq!(g.transmissions(), 3);
    // A tiny movement inside the same pixel column sends nothing.
    draw(&mut term, &mut g, &view(32.01, true));
    assert_eq!(g.transmissions(), 3);
}

#[test]
fn text_outside_the_waveform_is_untouched() {
    let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
    let mut g = Graphics::new(kitty());
    let buf = draw(&mut term, &mut g, &view(30.0, true));
    let text: String = (0..44)
        .map(|y| {
            (0..120)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect();
    for needle in ["▶ DECK A", "DECK B", "MIXER", "BROWSER", "PLAYING"] {
        assert!(text.contains(needle), "{needle} missing");
    }
}
