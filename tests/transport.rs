mod common;
use common::{stereo, write_wav, Fmt};
use dj_tui::app::App;
use dj_tui::config::Config;
use engine::{channel, DeckId, Engine, EngineProcessor};
use input::{Key, KeyEvent};
use std::time::{Duration, Instant};

const RATE: u32 = 48_000;

fn process(p: &mut EngineProcessor, frames: usize) {
    let (mut m, mut c) = (vec![0.0; frames * 2], vec![0.0; frames * 2]);
    p.process(&mut m, &mut c);
}

fn both_running(dir: &std::path::Path) -> (App, EngineProcessor) {
    let (handle, processor) = channel(Engine::new(), 64);
    let mut app = App::new(handle, &Config::default(), RATE);
    let mut p = processor;
    for (name, deck) in [("a.wav", DeckId::A), ("b.wav", DeckId::B)] {
        let path = dir.join(name);
        // Six seconds with a beat in it, so the detector gives both decks a real grid.
        let beat = 60.0 / 128.0;
        let mono: Vec<f32> = (0..RATE as usize * 6)
            .map(|i| {
                let tb = (i as f32 / RATE as f32) % beat;
                if tb < 0.02 {
                    (-tb / 0.004f32).exp() * (std::f32::consts::TAU * 1500.0 * tb).sin()
                } else {
                    0.0
                }
            })
            .collect();
        write_wav(&path, &stereo(&mono), 2, RATE, Fmt::Float32);
        app.load_path(deck, path);
        let deadline = Instant::now() + Duration::from_secs(20);
        while app.view(String::new()).decks[deck.index()].title.is_none() {
            app.tick();
            process(&mut p, 256);
            assert!(Instant::now() < deadline, "{name} never loaded");
        }
    }
    app.on_key(KeyEvent::press(Key::Space));
    app.on_key(KeyEvent::press(Key::Tab));
    app.on_key(KeyEvent::press(Key::Space));
    process(&mut p, 256);
    assert!(app.snapshot().decks[0].playing && app.snapshot().decks[1].playing);
    (app, p)
}

#[test]
fn stopping_one_deck_never_stops_the_other() {
    // The invariant: whatever else is set on either deck, pausing one leaves the other
    // running. Quantize is in here because that is the state it was reported under.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, mut p) = both_running(dir.path());
    // `both_running` leaves deck B focused.
    for key in [Key::Char('q'), Key::Space] {
        app.on_key(KeyEvent::press(key));
        app.tick();
        process(&mut p, 256);
    }
    let s = app.snapshot();
    assert!(!s.decks[1].playing, "deck B stopped");
    assert!(
        s.decks[0].playing,
        "deck A kept playing: stopping one deck must never take the music with it"
    );
}
