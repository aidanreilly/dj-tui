//! The raw ALSA backend. Enumeration runs anywhere; playback needs a card, so those tests
//! only run with DJ_TUI_ALSA_TESTS=1 set, the way the JACK ones do.

use backend::{alsa_devices, AlsaBackend, Routing};
use engine::{channel, Engine, OutputMode};

fn hardware_tests() -> bool {
    std::env::var("DJ_TUI_ALSA_TESTS").is_ok_and(|v| v == "1")
}

#[test]
fn the_device_list_describes_what_it_offers() {
    let devices = alsa_devices();
    // A machine with no sound card lists nothing, which is not a failure.
    for device in &devices {
        assert!(!device.name.is_empty(), "every device has a name to open");
        assert!(
            !device.description.is_empty(),
            "and something to show a person: {:?}",
            device.name
        );
    }
    assert!(
        devices
            .iter()
            .all(|d| !d.name.starts_with("hw:") || d.hardware),
        "hw: devices are the direct ones"
    );
    assert!(
        devices.iter().filter(|d| d.name == "default").count() <= 1,
        "the default is listed once"
    );
}

#[test]
fn opening_a_device_that_does_not_exist_says_so() {
    let err = match AlsaBackend::open("dj-tui-no-such-device", 48_000, 256) {
        Err(e) => e,
        Ok(_) => panic!("that device cannot exist"),
    };
    assert!(
        err.contains("dj-tui-no-such-device"),
        "the message names the device: {err}"
    );
}

#[test]
fn a_real_device_plays_and_reports_what_it_settled_on() {
    if !hardware_tests() {
        return;
    }
    let backend = AlsaBackend::open("default", 48_000, 256).expect("open the default device");
    assert!(backend.sample_rate() > 0);
    assert!(backend.channels() >= 2);

    let (_handle, processor) = channel(Engine::with_sample_rate(backend.sample_rate()), 16);
    let running = backend
        .activate(processor, &Routing::Auto, OutputMode::Mix)
        .expect("start playback");
    assert_eq!(running.device(), "default");
    assert!(running.buffer_size() > 0);
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(
        running.xruns(),
        0,
        "a quarter second of silence is not hard"
    );
    running.stop();
}

#[test]
fn deck_mode_needs_four_channels() {
    if !hardware_tests() {
        return;
    }
    let Ok(backend) = AlsaBackend::open("default", 48_000, 256) else {
        return;
    };
    if backend.channels() >= 4 {
        // This card can carry deck mode, so there is no error to assert.
        return;
    }
    let (_, processor) = channel(Engine::new(), 32);
    let Err(err) = backend.activate(processor, &Routing::Auto, OutputMode::Decks) else {
        panic!("deck mode should refuse a device with fewer than four channels");
    };
    assert!(err.contains("four output channels"), "{err}");
}
