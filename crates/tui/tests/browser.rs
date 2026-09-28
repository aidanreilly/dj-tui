//! The track browser: columns, selection, key highlighting and search.

use engine::DeckId;
use ratatui::{backend::TestBackend, style::Modifier, Terminal};
use tui::{render_screen, BrowserRow, BrowserView, DeckView, MixerView, ScreenView};

fn row(name: &str, bpm: Option<f64>, key: Option<&str>, secs: Option<f64>) -> BrowserRow {
    BrowserRow {
        name: name.into(),
        bpm,
        key: key.map(|k| k.to_string()),
        duration_secs: secs,
        compatible: false,
        analysed: bpm.is_some(),
    }
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

fn screen(browser: BrowserView) -> ScreenView {
    ScreenView {
        decks: [deck(DeckId::A, true), deck(DeckId::B, false)],
        mixer: MixerView::default(),
        status: String::new(),
        message: String::new(),
        phase: None,
        help: false,
        browser,
        devices: None,
    }
}

fn draw(view: &ScreenView, w: u16, h: u16) -> ratatui::buffer::Buffer {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render_screen(f, view)).unwrap();
    term.backend().buffer().clone()
}

fn text(buf: &ratatui::buffer::Buffer) -> String {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                + "\n"
        })
        .collect()
}

fn listing() -> BrowserView {
    BrowserView {
        rows: vec![
            row("Warehouse Tool", Some(128.0), Some("8A"), Some(312.0)),
            row("Breakdown Edit", Some(124.5), Some("9A"), Some(245.0)),
            row("Not Analysed Yet", None, None, None),
        ],
        selected: 0,
        sort: "name",
        ascending: true,
        search: None,
        active: false,
        fullscreen: false,
        status: "3 tracks".into(),
    }
}

#[test]
fn the_browser_lists_tracks_with_their_columns() {
    let t = text(&draw(&screen(listing()), 120, 44));
    assert!(t.contains("Warehouse Tool"), "{t}");
    assert!(t.contains("128.0"), "BPM to a decimal\n{t}");
    assert!(t.contains("8A"), "the key in Camelot\n{t}");
    assert!(t.contains("5:12"), "the length as minutes and seconds\n{t}");
    assert!(t.contains("3 tracks"), "and what the list holds\n{t}");
    assert!(t.contains("name"), "with the column it is sorted by\n{t}");
}

#[test]
fn nothing_found_says_so_rather_than_showing_an_empty_box() {
    let mut view = listing();
    view.rows.clear();
    view.status = "no tracks".into();
    let t = text(&draw(&screen(view), 120, 44));
    assert!(t.contains("no tracks"), "{t}");
}

#[test]
fn the_selected_row_stands_out() {
    let mut view = listing();
    view.selected = 1;
    let buf = draw(&screen(view), 120, 44);
    let row_of = |needle: &str| {
        (0..buf.area.height)
            .find(|&y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .contains(needle)
            })
            .expect("row on screen")
    };
    let selected = row_of("Breakdown Edit");
    let other = row_of("Warehouse Tool");
    assert!(
        buf[(3, selected)].modifier.contains(Modifier::REVERSED),
        "the row under the cursor is picked out"
    );
    assert!(!buf[(3, other)].modifier.contains(Modifier::REVERSED));
}

#[test]
fn a_track_in_a_compatible_key_is_coloured() {
    use ratatui::style::Color;
    let mut view = listing();
    view.rows[1].compatible = true;
    let buf = draw(&screen(view), 120, 44);
    let cell = (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .find(|&(x, y)| buf[(x, y)].symbol() == "9" && buf[(x + 1, y)].symbol() == "A")
        .expect("the key on screen");
    assert_eq!(
        buf[cell].fg,
        Color::Green,
        "it would mix with what is playing"
    );
}

#[test]
fn a_track_with_no_analysis_is_dimmed() {
    let buf = draw(&screen(listing()), 120, 44);
    let y = (0..buf.area.height)
        .find(|&y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .contains("Not Analysed Yet")
        })
        .unwrap();
    assert!(buf[(3, y)].modifier.contains(Modifier::DIM));
}

#[test]
fn a_long_list_scrolls_to_keep_the_selection_in_view() {
    let mut view = listing();
    view.rows = (0..200)
        .map(|i| {
            row(
                &format!("Track {i:03}"),
                Some(120.0),
                Some("8A"),
                Some(200.0),
            )
        })
        .collect();
    view.selected = 150;
    let t = text(&draw(&screen(view), 120, 44));
    assert!(t.contains("Track 150"), "the selection is on screen\n{t}");
    assert!(!t.contains("Track 000"), "and the top has scrolled away");
}

#[test]
fn searching_shows_what_has_been_typed() {
    let mut view = listing();
    view.search = Some("ware".into());
    let t = text(&draw(&screen(view), 120, 44));
    assert!(t.contains("/ware"), "{t}");
}

#[test]
fn full_screen_hides_the_decks() {
    let mut view = listing();
    view.fullscreen = true;
    let t = text(&draw(&screen(view), 120, 44));
    assert!(t.contains("Warehouse Tool"));
    assert!(!t.contains("DECK A"), "the browser has the screen\n{t}");
    assert!(!t.contains("MIXER"), "{t}");
}

#[test]
fn the_browser_survives_a_tiny_terminal() {
    for (w, h) in [(30, 8), (60, 20)] {
        draw(&screen(listing()), w, h);
        let mut full = listing();
        full.fullscreen = true;
        draw(&screen(full), w, h);
    }
}

mod devices {
    use super::*;
    use tui::DeviceView;

    fn with_devices(view: DeviceView) -> ScreenView {
        let mut screen = screen(Default::default());
        screen.devices = Some(view);
        screen
    }

    fn listing() -> DeviceView {
        DeviceView {
            devices: vec![
                ("hw:0,0".into(), "Built-in Audio".into()),
                ("default".into(), "Default ALSA Output".into()),
            ],
            selected: 0,
            current: "default".into(),
            note: "48000 Hz / 256 frames".into(),
        }
    }

    #[test]
    fn the_device_screen_lists_what_can_be_opened() {
        let t = text(&draw(&with_devices(listing()), 120, 44));
        assert!(t.contains("AUDIO DEVICE"), "{t}");
        assert!(t.contains("hw:0,0"), "{t}");
        assert!(t.contains("Built-in Audio"), "{t}");
        assert!(t.contains("48000 Hz"), "what is running now\n{t}");
    }

    #[test]
    fn the_device_in_use_is_marked_and_the_selection_stands_out() {
        use ratatui::style::Modifier;
        let mut view = listing();
        view.selected = 1;
        let buf = draw(&with_devices(view), 120, 44);
        let row_of = |needle: &str| {
            (0..buf.area.height)
                .find(|&y| {
                    (0..buf.area.width)
                        .map(|x| buf[(x, y)].symbol().to_string())
                        .collect::<String>()
                        .contains(needle)
                })
                .expect("row on screen")
        };
        let default_row = row_of("Default ALSA Output");
        let line: String = (0..buf.area.width)
            .map(|x| buf[(x, default_row)].symbol().to_string())
            .collect();
        assert!(line.contains('●'), "the one in use is marked: {line}");
        assert!(
            (0..buf.area.width)
                .any(|x| buf[(x, default_row)].modifier.contains(Modifier::REVERSED)),
            "and the selection is picked out"
        );
    }

    #[test]
    fn no_devices_says_so() {
        let mut view = listing();
        view.devices.clear();
        let t = text(&draw(&with_devices(view), 120, 44));
        assert!(t.contains("No audio devices"), "{t}");
    }

    #[test]
    fn the_device_screen_fits_a_small_terminal() {
        for (w, h) in [(30, 8), (60, 20)] {
            draw(&with_devices(listing()), w, h);
        }
    }
}

#[test]
fn the_panel_says_when_it_holds_the_keyboard() {
    let mut view = listing();
    let quiet = text(&draw(&screen(view.clone()), 120, 44));
    assert!(
        !quiet.contains("Alt+s"),
        "no hint line when it is not active"
    );

    view.active = true;
    view.search = Some("ac".into());
    let busy = text(&draw(&screen(view), 120, 44));
    assert!(
        busy.contains("Alt+s"),
        "the commands are on screen while typing"
    );
    assert!(busy.contains("Esc"), "and so is the way out");
    assert!(busy.contains("ac"), "along with the query");
}

#[test]
fn the_help_list_matches_the_keys_that_exist() {
    let mut view = screen(listing());
    view.help = true;
    let shown = text(&draw(&view, 120, 44));
    for gone in ["send the next key", "channel fader", "Alt to kill"] {
        assert!(!shown.contains(gone), "stale entry: {gone}");
    }
    for present in ["x / X", "t / y / u", "T / Y / U", "{ / }", "Alt+arrows"] {
        assert!(shown.contains(present), "missing entry: {present}");
    }
}
