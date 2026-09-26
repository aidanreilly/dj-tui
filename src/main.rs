//! dj-tui entry point: config, audio backend, terminal setup and the UI loop.

use backend::JackBackend;
use dj_tui::{
    app::App,
    cli::{parse_args, USAGE},
    clock::NullClock,
    config::{config_path, Backend, Config},
    demo::{click_track, planar},
    view::DeckMeta,
};
use engine::{channel, DeckId, Engine, EngineProcessor};
use ratatui::crossterm::{
    event::{
        self, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::supports_keyboard_enhancement,
};
use std::{
    io::stdout,
    process::ExitCode,
    time::{Duration, Instant},
};

const FRAME: Duration = Duration::from_millis(33);
const COMMAND_QUEUE: usize = 256;

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

/// Where audio goes: a live JACK client, or a silent clock we drive from the UI loop.
enum Audio {
    Jack(backend::Running),
    Silent(NullClock, EngineProcessor),
}

impl Audio {
    fn status(&self) -> String {
        match self {
            Audio::Jack(r) => format!(
                "JACK {} @ {} Hz / {} frames, xruns {}",
                r.client_name(),
                r.sample_rate(),
                r.buffer_size(),
                r.xruns()
            ),
            Audio::Silent(..) => "no audio output".into(),
        }
    }
}

fn start_audio(config: &Config, no_audio: bool) -> Result<(App, Audio, Vec<String>), String> {
    let mut notes = Vec::new();
    if config.audio.backend == Backend::Alsa {
        notes.push("raw ALSA backend arrives in M10; using JACK".to_string());
    }
    let routing = config.routing()?;
    let jack = if no_audio {
        None
    } else {
        match JackBackend::open(&config.audio.client_name) {
            Ok(j) => Some(j),
            Err(e) => {
                notes.push(format!("{e}; running silent"));
                None
            }
        }
    };
    let (handle, processor) = channel(Engine::new(), COMMAND_QUEUE);
    Ok(match jack {
        Some(jack) => {
            let rate = jack.sample_rate();
            let app = App::new(handle, config, rate);
            let running = jack.activate(processor, &routing)?;
            notes.extend(running.warnings().iter().cloned());
            (app, Audio::Jack(running), notes)
        }
        None => {
            let rate = config.audio.sample_rate;
            (
                App::new(handle, config, rate),
                Audio::Silent(NullClock::new(rate), processor),
                notes,
            )
        }
    })
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("dj-tui: {e}");
            return ExitCode::from(2);
        }
    };
    if args.help {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.version {
        println!("dj-tui {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dj-tui: config error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (mut app, mut audio, notes) = match start_audio(&config, args.no_audio) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("dj-tui: audio: {e}");
            return ExitCode::FAILURE;
        }
    };

    if args.demo && args.files.is_empty() {
        for (id, bpm) in [(DeckId::A, 124.0), (DeckId::B, 126.0)] {
            let track = click_track(bpm, 180.0, app.sample_rate());
            let [l, r] = planar(&track);
            let waveform = wave::analyse(&l, &r, app.sample_rate());
            let meta = DeckMeta {
                title: Some(format!("Demo click {bpm:.0}")),
                bpm: Some(bpm),
                key: None,
                loading: false,
                waveform,
            };
            app.load_track(id, track, meta);
        }
    }
    for (path, deck) in args.files.into_iter().zip([DeckId::A, DeckId::B]) {
        app.load_path(deck, path);
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
    let mut graphics = match config.ui.graphics {
        dj_tui::config::Graphics::Auto => tui::detect_graphics().map(tui::Graphics::new),
        dj_tui::config::Graphics::Off => None,
    };
    let keys = if key_release {
        "hold-cue on"
    } else {
        "cue is press only (no kitty keyboard)"
    };
    let mut notes = notes.into_iter();
    let mut note = notes.next();
    let mut note_shown = Instant::now();
    let mut last = Instant::now();

    let result = (|| -> std::io::Result<()> {
        loop {
            if event::poll(FRAME)? {
                if let Event::Key(k) = event::read()? {
                    if tui::convert_key(k).is_some_and(|k| app.on_key(k)) {
                        return Ok(());
                    }
                }
            }
            app.tick();
            let now = Instant::now();
            if let Audio::Silent(clock, processor) = &mut audio {
                clock.advance(processor, now - last);
            }
            last = now;
            // Startup notes rotate every four seconds, then the line returns to normal.
            if note.is_some() && note_shown.elapsed() > Duration::from_secs(4) {
                note = notes.next();
                note_shown = now;
            }
            let status = match &note {
                Some(n) => format!("{}  |  {n}", audio.status()),
                None => format!(
                    "{}  |  {keys}  |  {}  |  ? help  Ctrl+Q quit",
                    audio.status(),
                    if graphics.is_some() {
                        "pixel waveforms"
                    } else {
                        "glyph waveforms"
                    }
                ),
            };
            let view = app.view(status);
            terminal.draw(|f| {
                tui::render_screen(f, &view);
                if let Some(g) = &mut graphics {
                    g.render(f, &view);
                }
            })?;
        }
    })();

    if key_release {
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    if let Audio::Jack(running) = audio {
        running.stop();
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dj-tui: {e}");
            ExitCode::FAILURE
        }
    }
}
