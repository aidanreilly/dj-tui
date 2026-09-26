//! Runs against a real JACK server. Skipped unless DJ_TUI_JACK_TESTS=1, since CI machines
//! and laptops don't always have one. Start a server with:
//!     jackd -d dummy -r 48000 -p 256 -P 4

use backend::{JackBackend, Routing};
use engine::{channel, Command, DeckId::A, Engine, Track};
use std::{sync::Arc, time::Duration};

fn enabled() -> bool {
    std::env::var("DJ_TUI_JACK_TESTS").as_deref() == Ok("1")
}

#[test]
fn plays_through_a_real_jack_server() {
    if !enabled() {
        eprintln!("skipping: set DJ_TUI_JACK_TESTS=1 with jackd running");
        return;
    }
    let jack = JackBackend::open("dj-tui-test").expect("jack server running");
    let rate = jack.sample_rate();
    assert!(rate > 0);
    let (mut h, p) = channel(Engine::new(), 32);
    let running = jack.activate(p, &Routing::Auto).expect("activate");
    assert!(running.warnings().is_empty(), "{:?}", running.warnings());

    let ports = running.connected_ports();
    assert!(ports.iter().any(|(ours, theirs)| ours == "dj-tui-test:master_L" && theirs == "system:playback_1"), "{ports:?}");

    h.send(Command::Load(A, Arc::new(Track::from_interleaved(vec![0.1; rate as usize * 4], rate)))).unwrap();
    h.send(Command::PlayPause(A)).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let pos = h.snapshot().decks[0].position;
    assert!(pos > rate as f64 * 0.1, "position only {pos}");
    assert_eq!(running.xruns(), running.xruns(), "xrun counter readable");
    h.collect_garbage();
    running.stop();
}
