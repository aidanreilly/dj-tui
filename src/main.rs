//! dj-tui entry point: config, audio backend, terminal setup and the UI loop.

use backend::JackBackend;
use dj_tui::{
    app::App,
    cli::{parse_args, USAGE},
    clock::NullClock,
    config::{config_path, Backend, Config},
    demo::{click_track, waveform_envelope},
    view::DeckMeta,
};
use engine::{channel, DeckId, Engine, EngineProcessor};
use loader::ENVELOPE_POINTS;
use ratatui::crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyboardEnhancementFlags,
        MouseButton, MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::supports_keyboard_enhancement,
};
use ratatui::layout::Rect;
use std::{
    io::stdout,
    process::ExitCode,
    time::{Duration, Instant},
};

const FRAME: Duration = Duration::from_millis(33);
const COMMAND_QUEUE: usize = 256;
/// How often to look for controllers that have been plugged in since the last look.
const MIDI_SCAN: Duration = Duration::from_secs(2);

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

/// Print what a controller sends, so its numbers can go into a mapping file. Runs until
/// Ctrl+C, since there is no way to know when the user has finished pressing things.
fn midi_learn() -> ExitCode {
    let backend = match backend::JackBackend::open("dj-tui-learn") {
        Ok(b) => b,
        Err(e) => {
            eprintln!("MIDI learn needs a running JACK or PipeWire server: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (_, processor) = engine::channel(Engine::new(), COMMAND_QUEUE);
    let mut running = match backend.activate(processor, &backend::Routing::Auto) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    println!("Listening for controllers. Press a control to see what it sends; Ctrl+C to stop.");
    println!("Paste the lines into a mapping file under mappings/, changing the action to suit.");
    let mut last = None;
    loop {
        for port in running.connect_midi(&[]) {
            println!("connected {port}");
        }
        for bytes in running.take_midi() {
            let Some(message) = midi::Message::from_bytes(&bytes) else {
                continue;
            };
            let address = message.address();
            // A knob sends a stream of values; only its first message is worth printing.
            if last.as_deref() == Some(address.as_str()) {
                continue;
            }
            last = Some(address.clone());
            println!();
            println!("{address}");
            print!("{}", midi::learn_line(message, "play a"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Where audio goes: a live JACK client, or a silent clock we drive from the UI loop.
#[allow(clippy::large_enum_variant)] // one value for the whole run; boxing buys nothing
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
    // The engine's filters need the real session rate, which JACK decides.
    let rate = jack
        .as_ref()
        .map_or(config.audio.sample_rate, JackBackend::sample_rate);
    let (handle, processor) = channel(Engine::with_sample_rate(rate), COMMAND_QUEUE);
    Ok(match jack {
        Some(jack) => {
            let app = App::new(handle, config, rate);
            let running = jack.activate(processor, &routing)?;
            notes.extend(running.warnings().iter().cloned());
            (app, Audio::Jack(running), notes)
        }
        None => (
            App::new(handle, config, rate),
            Audio::Silent(NullClock::new(rate), processor),
            notes,
        ),
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
    if args.midi_learn {
        return midi_learn();
    }
    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dj-tui: config error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (mut app, mut audio, mut notes) = match start_audio(&config, args.no_audio) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("dj-tui: audio: {e}");
            return ExitCode::FAILURE;
        }
    };

    if args.demo && args.files.is_empty() {
        for (id, bpm) in [(DeckId::A, 124.0), (DeckId::B, 126.0)] {
            let track = click_track(bpm, 180.0, app.sample_rate());
            let waveform = waveform_envelope(&track, ENVELOPE_POINTS);
            let bands = loader::band_envelope(&track, ENVELOPE_POINTS);
            let meta = DeckMeta {
                bands,
                grid: Some(analysis::tempo::BeatGrid {
                    bpm,
                    first_beat_secs: 0.0,
                }),
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

    // Controllers: mappings come from `mappings/` beside the config file.
    let mapping_dir = config_path(
        std::env::var("XDG_CONFIG_HOME").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    )
    .and_then(|p| p.parent().map(|d| d.join("mappings")));
    let (mut controllers, midi_problems) =
        dj_tui::controller::load_mappings(&config.midi, mapping_dir.as_deref());
    for problem in midi_problems {
        notes.push(format!("MIDI mapping: {problem}"));
    }
    if !controllers.is_empty() {
        notes.push(format!(
            "MIDI mappings loaded: {}",
            controllers
                .iter()
                .map(|c| c.name().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let midi_ports: Vec<String> = controllers.iter().flat_map(|c| c.wanted_ports()).collect();
    let mut midi_scan = Instant::now();

    let mut terminal = ratatui::init();
    let mouse_capture = execute!(stdout(), EnableMouseCapture).is_ok();
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
                match event::read()? {
                    Event::Key(k) => {
                        if tui::convert_key(k).is_some_and(|k| app.on_key(k)) {
                            return Ok(());
                        }
                    }
                    Event::Mouse(mouse)
                        if mouse.kind == MouseEventKind::Down(MouseButton::Left) =>
                    {
                        let size = terminal.size()?;
                        let layout = tui::screen_layout(Rect::new(0, 0, size.width, size.height));
                        for (deck, panel) in
                            [(DeckId::A, layout.deck_a), (DeckId::B, layout.deck_b)]
                        {
                            let area = tui::waveform_area(panel);
                            if mouse.column >= area.x
                                && mouse.column < area.right()
                                && mouse.row >= area.y
                                && mouse.row < area.bottom()
                            {
                                let column = mouse.column - area.x;
                                let last = area.width.saturating_sub(1);
                                let fraction = if last == 0 {
                                    0.0
                                } else {
                                    column as f64 / last as f64
                                };
                                app.seek_to_fraction(deck, fraction);
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Audio::Jack(running) = &mut audio {
                if !controllers.is_empty() {
                    // Rescanning every couple of seconds is what makes hotplug work.
                    if midi_scan.elapsed() > MIDI_SCAN {
                        for port in running.connect_midi(&midi_ports) {
                            app.note(format!("Controller connected: {port}"));
                        }
                        midi_scan = Instant::now();
                    }
                    for bytes in running.take_midi() {
                        for controller in &mut controllers {
                            if controller.handle(&mut app, bytes) {
                                return Ok(());
                            }
                        }
                    }
                    for controller in &mut controllers {
                        controller.sync(&app);
                        for message in controller.lights(&app) {
                            running.send_midi(message.to_bytes());
                        }
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
                    "{}  |  {keys}  |  click waveform to seek  |  {}  |  ? help  Ctrl+Q quit",
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
    if mouse_capture {
        let _ = execute!(stdout(), DisableMouseCapture);
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
