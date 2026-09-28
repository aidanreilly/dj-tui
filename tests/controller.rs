//! Mappings loaded from disk, and a controller driving the app.

mod common;
use common::{stereo, write_wav, Fmt};
use dj_tui::app::App;
use dj_tui::config::{Config, Midi};
use dj_tui::controller::{load_mappings, Controller};
use engine::{channel, DeckId, Engine, EngineProcessor};
use midi::{Mapping, Message};

const RATE: u32 = 48_000;

fn app() -> (App, EngineProcessor) {
    let (handle, processor) = channel(Engine::new(), 64);
    (App::new(handle, &Config::default(), RATE), processor)
}

fn process(p: &mut EngineProcessor, frames: usize) {
    let (mut m, mut c) = (vec![0.0; frames * 2], vec![0.0; frames * 2]);
    p.process(&mut m, &mut c);
}

const MAPPING: &str = r#"
name = "Test rig"
ports = ["Test Controller"]
[[buttons]]
input = "note 0 11"
action = "play a"
[[buttons]]
input = "note 0 99"
action = "quit"
[[knobs]]
input = "cc 0 19"
control = "fader a"
[[leds]]
output = "note 0 11"
state = "playing a"
"#;

fn rig(soft_takeover: bool) -> Controller {
    Controller::new(Mapping::from_toml(MAPPING).unwrap(), soft_takeover)
}

#[test]
fn the_shipped_mapping_is_valid() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/mappings/generic.toml"
    ))
    .unwrap();
    let mapping = Mapping::from_toml(&text).expect("the mapping we ship parses");
    // What the mapping calls itself is the label on a starting point people copy and edit,
    // so this checks it has one rather than pinning the words.
    assert!(
        !mapping.name().is_empty(),
        "a mapping needs a name to report"
    );
    assert!(mapping.leds().count() >= 6, "and lights up what it can");
}

#[test]
fn a_button_on_the_controller_plays_the_deck() {
    let (mut app, mut p) = app();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.wav");
    write_wav(&path, &stereo(&[0.3; 48_000]), 2, RATE, Fmt::Pcm16);
    app.load_path(DeckId::A, path);
    let start = std::time::Instant::now();
    while app.snapshot().decks[0].track_frames == 0 {
        app.tick();
        process(&mut p, 16);
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let mut rig = rig(false);
    let play = Message::NoteOn {
        channel: 0,
        note: 11,
        velocity: 127,
    };
    assert!(!rig.handle(&mut app, play.to_bytes()));
    process(&mut p, 16);
    assert!(app.snapshot().decks[0].playing, "the deck is running");
}

#[test]
fn a_quit_binding_stops_the_app() {
    let (mut app, _p) = app();
    let mut rig = rig(false);
    assert!(rig.handle(
        &mut app,
        Message::NoteOn {
            channel: 0,
            note: 99,
            velocity: 127
        }
        .to_bytes()
    ));
}

#[test]
fn a_fader_with_takeover_waits_for_the_value_on_screen() {
    let (mut app, mut p) = app();
    let mut rig = rig(true);
    // The app starts with the fader up; the controller's is at the bottom.
    rig.sync(&app);
    let low = Message::Cc {
        channel: 0,
        controller: 19,
        value: 0,
    };
    rig.handle(&mut app, low.to_bytes());
    process(&mut p, 16);
    assert_eq!(app.snapshot().faders[0], 1.0, "nothing moved yet");

    let matched = Message::Cc {
        channel: 0,
        controller: 19,
        value: 127,
    };
    rig.handle(&mut app, matched.to_bytes());
    process(&mut p, 16);
    rig.handle(&mut app, low.to_bytes());
    process(&mut p, 16);
    assert_eq!(app.snapshot().faders[0], 0.0, "and follows once it has");
}

#[test]
fn the_lights_follow_the_decks() {
    let (mut app, mut p) = app();
    let mut rig = rig(false);
    let first = rig.lights(&app);
    assert!(!first.is_empty(), "every light is set at the start");
    assert!(rig.lights(&app).is_empty(), "then only on a change");

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.wav");
    write_wav(&path, &stereo(&[0.3; 48_000]), 2, RATE, Fmt::Pcm16);
    app.load_path(DeckId::A, path);
    let start = std::time::Instant::now();
    while app.snapshot().decks[0].track_frames == 0 {
        app.tick();
        process(&mut p, 16);
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app.on_action(input::Action::PlayPause(DeckId::A));
    process(&mut p, 16);
    assert_eq!(
        rig.lights(&app),
        vec![Message::NoteOn {
            channel: 0,
            note: 11,
            velocity: 127
        }]
    );
}

#[test]
fn mappings_load_from_a_directory_and_bad_ones_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("good.toml"), MAPPING).unwrap();
    std::fs::write(
        dir.path().join("bad.toml"),
        "name = \"Bad\"\n[[buttons]]\ninput = \"note 0 1\"\naction = \"fly a\"\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("notes.txt"), "ignored").unwrap();

    let (loaded, problems) = load_mappings(&Midi::default(), Some(dir.path()));
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name(), "Test rig");
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("fly a"), "{:?}", problems[0]);
}

#[test]
fn midi_switched_off_loads_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("good.toml"), MAPPING).unwrap();
    let config = Midi {
        enabled: false,
        ..Default::default()
    };
    let (loaded, problems) = load_mappings(&config, Some(dir.path()));
    assert!(loaded.is_empty() && problems.is_empty());
}

#[test]
fn a_named_mapping_is_the_only_one_loaded() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("one.toml"), MAPPING).unwrap();
    std::fs::write(dir.path().join("two.toml"), MAPPING).unwrap();
    let config = Midi {
        mappings: vec!["two.toml".into()],
        ..Default::default()
    };
    let (loaded, problems) = load_mappings(&config, Some(dir.path()));
    assert_eq!(loaded.len(), 1, "{problems:?}");
}
