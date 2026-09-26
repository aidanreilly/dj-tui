//! dj-tui entry point: config, terminal setup, and the UI loop.
//!
//! M0 runs the engine on a silent `NullClock`. The JACK backend replaces it in M1.

use dj_tui::{
    apply::{apply, Controls},
    clock::NullClock,
    config::{config_path, Config},
    demo::{click_track, peak_envelope},
    view::{screen_view, DeckMeta},
};
use engine::{DeckId, Engine};
use input::{Action, Keymap};
use ratatui::crossterm::{
    event::{self, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    execute,
    terminal::supports_keyboard_enhancement,
};
use std::{
    io::stdout,
    process::ExitCode,
    sync::Arc,
    time::{Duration, Instant},
};

const FRAME: Duration = Duration::from_millis(33);
const ENVELOPE_POINTS: usize = 2048;

fn load_config() -> Result<Config, String> {
    let path = config_path(
        std::env::var("XDG_CONFIG_HOME").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    );
    match path {
        Some(p) if p.exists() => {
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            Config::from_toml(&text).map_err(|e| format!("{}: {e}", p.display()))
        }
        _ => Ok(Config::default()),
    }
}

fn load_demo(engine: &mut Engine, metas: &mut [DeckMeta; 2], rate: u32) {
    for (i, (id, bpm)) in [(DeckId::A, 124.0), (DeckId::B, 126.0)].into_iter().enumerate() {
        let track = Arc::new(click_track(bpm, 180.0, rate));
        metas[i] = DeckMeta {
            title: Some(format!("Demo click {bpm:.0}")),
            bpm: Some(bpm),
            key: None,
            envelope: peak_envelope(&track, ENVELOPE_POINTS),
        };
        engine.deck_mut(id).load(track);
    }
}

fn main() -> ExitCode {
    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dj-tui: config error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let demo = std::env::args().any(|a| a == "--demo");

    let mut engine = Engine::new();
    engine.set_crossfader_curve(config.mixer.crossfader_curve);
    let mut metas: [DeckMeta; 2] = Default::default();
    if demo {
        load_demo(&mut engine, &mut metas, config.audio.sample_rate);
    }

    let mut terminal = ratatui::init();
    let key_release = supports_keyboard_enhancement().unwrap_or(false)
        && execute!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
            )
        )
        .is_ok();
    let status = format!(
        "{}  |  audio: none (M0)  |  ? help  Ctrl+Q quit",
        if key_release { "keys: kitty protocol, hold-cue on" } else { "keys: legacy, cue is press only" }
    );

    let controls = Controls::new(config.deck.tempo_range);
    let mut keymap = Keymap::new();
    let mut clock = NullClock::new(config.audio.sample_rate);
    let mut last = Instant::now();

    let result = (|| -> std::io::Result<()> {
        loop {
            if event::poll(FRAME)? {
                if let Event::Key(k) = event::read()? {
                    if let Some(action) = tui::convert_key(k).and_then(|k| keymap.handle(k)) {
                        if action == Action::Quit {
                            return Ok(());
                        }
                        apply(&mut engine, &controls, action);
                    }
                }
            }
            let now = Instant::now();
            clock.advance(&mut engine, now - last);
            last = now;
            let view = screen_view(&engine, keymap.focused(), &metas, status.clone());
            terminal.draw(|f| tui::render_screen(f, &view))?;
        }
    })();

    if key_release {
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dj-tui: {e}");
            ExitCode::FAILURE
        }
    }
}
