//! UI-side application state. Owns the engine handle, keymap, control values, track
//! metadata and the background loader. Everything here runs on the UI thread.

use crate::apply::{apply, ControlState, Controls};
use crate::config::Config;
use crate::view::{screen_view, DeckMeta};
use engine::{Command, DeckId, EngineHandle, Snapshot};
use input::{Action, KeyEvent, Keymap};
use loader::Loader;
use std::path::PathBuf;
use std::sync::Arc;
use tui::ScreenView;

pub struct App {
    handle: EngineHandle,
    controls: Controls,
    state: ControlState,
    keymap: Keymap,
    metas: [DeckMeta; 2],
    loader: Loader,
    sample_rate: u32,
    message: String,
}

impl App {
    /// `sample_rate` is the session rate reported by the audio backend.
    pub fn new(handle: EngineHandle, config: &Config, sample_rate: u32) -> Self {
        let mut app = Self {
            handle,
            controls: Controls::new(config.deck.tempo_range),
            state: ControlState::default(),
            keymap: Keymap::new(),
            metas: Default::default(),
            loader: Loader::spawn(sample_rate),
            sample_rate,
            message: String::new(),
        };
        app.send(Command::SetCrossfaderCurve(config.mixer.crossfader_curve));
        app
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Queue a file for background loading onto `deck`.
    pub fn load_path(&mut self, deck: DeckId, path: PathBuf) {
        self.message = format!("Loading {} on deck {}…", path.display(), deck_letter(deck));
        self.loader.request(deck, path);
    }

    /// Put an already decoded track on a deck (demo mode, tests).
    pub fn load_track(&mut self, deck: DeckId, track: engine::Track, meta: DeckMeta) {
        self.metas[deck.index()] = meta;
        self.send(Command::Load(deck, Arc::new(track)));
    }

    /// Call once per UI frame: frees replaced tracks and picks up finished loads.
    pub fn tick(&mut self) {
        self.handle.collect_garbage();
        while let Some(done) = self.loader.try_recv() {
            let name = done.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            match done.result {
                Ok(loaded) => {
                    let title = match &loaded.artist {
                        Some(a) => format!("{a} - {}", loaded.title),
                        None => loaded.title.clone(),
                    };
                    self.message = format!("Loaded {title} on deck {}", deck_letter(done.deck));
                    self.load_track(
                        done.deck,
                        loaded.track,
                        DeckMeta { title: Some(title), bpm: None, key: None, envelope: loaded.envelope },
                    );
                }
                Err(e) => self.message = format!("Could not load {name}: {e}"),
            }
        }
    }

    /// Handle one key event. Returns true when the user asked to quit.
    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        let Some(action) = self.keymap.handle(key) else { return false };
        match action {
            Action::Quit => return true,
            Action::Load(_) => {
                self.message = "The library browser arrives in M8; pass files on the command line for now".into();
            }
            _ => {
                let snap = self.handle.snapshot();
                if let Some(cmd) = apply(&mut self.state, &self.controls, &snap, action) {
                    self.send(cmd);
                }
            }
        }
        false
    }

    pub fn snapshot(&self) -> Snapshot {
        self.handle.snapshot()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// `status` is prepended to the current message on the status line.
    pub fn view(&self, status: String) -> ScreenView {
        let line = match (status.is_empty(), self.message.is_empty()) {
            (false, false) => format!("{status}  |  {}", self.message),
            (false, true) => status,
            _ => self.message.clone(),
        };
        screen_view(&self.handle.snapshot(), self.sample_rate, self.keymap.focused(), &self.metas, line)
    }

    fn send(&mut self, cmd: Command) {
        if self.handle.send(cmd).is_err() {
            self.message = "Audio engine is not keeping up (command queue full)".into();
        }
    }
}

fn deck_letter(d: DeckId) -> char {
    match d {
        DeckId::A => 'A',
        DeckId::B => 'B',
    }
}
