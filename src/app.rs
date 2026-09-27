//! UI-side application state. Owns the engine handle, keymap, control values, track
//! metadata and the background loader. Everything here runs on the UI thread.

use crate::apply::{apply, ControlState, Controls};
use crate::config::Config;
use crate::view::{screen_view, DeckMeta};
use engine::{Command, DeckId, EngineHandle, Snapshot};
use input::{Action, KeyEvent, Keymap};
use loader::sidecar::Cues;
use loader::Loader;
use std::path::PathBuf;
use std::sync::Arc;
use tui::pixel::WaveformMode;
use tui::ScreenView;

/// Meter fall per UI frame. At 30 frames a second this is about 20 dB a second.
const METER_FALL_PER_TICK: f32 = 0.926;

pub struct App {
    handle: EngineHandle,
    controls: Controls,
    state: ControlState,
    keymap: Keymap,
    metas: [DeckMeta; 2],
    loader: Loader,
    sample_rate: u32,
    message: String,
    /// Decaying peak meters: deck A, deck B, master.
    meters: [f32; 3],
    waveform_mode: WaveformMode,
    end_warning_secs: u32,
    started: std::time::Instant,
    /// Where each deck's track came from, and the cues last saved beside it.
    persisted: [Option<Persisted>; 2],
}

struct Persisted {
    path: PathBuf,
    cues: Cues,
    /// Engine frames processed when the track and its cues were sent. Snapshots from before
    /// that point still describe the previous track, so they must not be saved.
    after_frames: u64,
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
            meters: [0.0; 3],
            end_warning_secs: config.ui.end_warning_secs,
            started: std::time::Instant::now(),
            persisted: [None, None],
            waveform_mode: match config.ui.waveform_mode {
                crate::config::WaveformMode::ThreeBand => WaveformMode::ThreeBand,
                crate::config::WaveformMode::Rgb => WaveformMode::Rgb,
                crate::config::WaveformMode::Blue => WaveformMode::Blue,
            },
        };
        app.send(Command::SetCrossfaderCurve(config.mixer.crossfader_curve));
        app
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Queue a file for background loading onto `deck`.
    pub fn load_path(&mut self, deck: DeckId, path: PathBuf) {
        self.metas[deck.index()].loading = true;
        self.message = format!("Loading {} on deck {}…", path.display(), deck_letter(deck));
        self.loader.request(deck, path);
    }

    /// Put an already decoded track on a deck (demo mode, tests).
    pub fn load_track(&mut self, deck: DeckId, track: engine::Track, meta: DeckMeta) {
        self.metas[deck.index()] = DeckMeta {
            loading: false,
            ..meta
        };
        self.send(Command::Load(deck, Arc::new(track)));
    }

    /// Call once per UI frame: frees replaced tracks and picks up finished loads.
    pub fn tick(&mut self) {
        self.handle.collect_garbage();
        self.save_changed_cues();
        while let Some(note) = self.loader.try_recv_note() {
            self.message = note;
        }
        let fresh = self.handle.take_meters();
        for (held, new) in
            self.meters
                .iter_mut()
                .zip([fresh.channels[0], fresh.channels[1], fresh.master])
        {
            *held = new.max(*held * METER_FALL_PER_TICK);
            if *held < 1e-4 {
                *held = 0.0;
            }
        }
        while let Some(done) = self.loader.try_recv() {
            let name = done
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match done.result {
                Ok(loaded) => {
                    let title = match &loaded.artist {
                        Some(a) => format!("{a} - {}", loaded.title),
                        None => loaded.title.clone(),
                    };
                    self.message = match &loaded.sidecar_note {
                        Some(note) => format!(
                            "Loaded {title} on deck {}, but {note}",
                            deck_letter(done.deck)
                        ),
                        None => format!("Loaded {title} on deck {}", deck_letter(done.deck)),
                    };
                    let cues = loaded.cues.clone();
                    self.load_track(
                        done.deck,
                        loaded.track,
                        DeckMeta {
                            title: Some(title),
                            bpm: loaded.grid.map(|g| g.bpm),
                            key: loaded.key.map(|k| k.camelot()),
                            loading: false,
                            waveform: loaded.waveform,
                            bands: loaded.bands,
                            grid: loaded.grid,
                        },
                    );
                    self.restore_cues(done.deck, done.path, cues);
                }
                Err(e) => {
                    self.metas[done.deck.index()].loading = false;
                    self.message = format!("Could not load {name}: {e}");
                }
            }
        }
    }

    fn restore_cues(&mut self, deck: DeckId, path: PathBuf, cues: Cues) {
        let rate = self.sample_rate as f64;
        if let Some(secs) = cues.main_cue_secs {
            self.send(Command::SetCuePoint(deck, secs * rate));
        }
        for (n, secs) in cues.hot_cues.iter().enumerate() {
            if let Some(secs) = secs {
                self.send(Command::SetHotCue(deck, n, Some(secs * rate)));
            }
        }
        let after_frames = self.handle.snapshot().frames_processed;
        self.persisted[deck.index()] = Some(Persisted {
            path,
            cues,
            after_frames,
        });
    }

    /// Save cues that changed since the last save. Positions are compared in seconds.
    fn save_changed_cues(&mut self) {
        let snap = self.handle.snapshot();
        let rate = self.sample_rate as f64;
        for (i, slot) in self.persisted.iter_mut().enumerate() {
            let Some(p) = slot else { continue };
            let d = &snap.decks[i];
            if snap.frames_processed <= p.after_frames || d.track_frames == 0 {
                continue;
            }
            let now = Cues {
                // A main cue at the very start is the default, not a choice worth saving.
                main_cue_secs: (d.cue_point > 0.0).then(|| d.cue_point / rate),
                hot_cues: d.hot_cues.map(|c| c.map(|f| f / rate)),
            };
            if now != p.cues {
                self.loader.save_cues(p.path.clone(), now.clone());
                p.cues = now;
            }
        }
    }

    /// Handle one key event. Returns true when the user asked to quit.
    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        let Some(action) = self.keymap.handle(key) else {
            return false;
        };
        match action {
            Action::Quit => return true,
            Action::CycleWaveformMode => {
                self.waveform_mode = self.waveform_mode.next();
                self.message = format!("Waveform: {}", mode_name(self.waveform_mode));
            }
            Action::Load(_) => {
                self.message =
                    "The library browser arrives in M8; pass files on the command line for now"
                        .into();
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

    pub fn view(&self, status: String) -> ScreenView {
        let mut v = screen_view(
            &self.handle.snapshot(),
            self.sample_rate,
            self.keymap.focused(),
            &self.metas,
            status,
        );
        v.message = self.message.clone();
        let elapsed = self.started.elapsed().as_secs_f64();
        for d in &mut v.decks {
            d.waveform_mode = self.waveform_mode;
            d.end_warning = crate::view::end_warning(
                d.duration_secs - d.position_secs,
                d.playing,
                self.end_warning_secs,
                elapsed,
            );
        }
        for (i, (view, st)) in v
            .mixer
            .strips
            .iter_mut()
            .zip(&self.state.strips)
            .enumerate()
        {
            *view = tui::StripView {
                trim_db: st.trim_db,
                eq_db: st.eq_db,
                kills: st.kills,
                filter: st.filter,
                meter: self.meters[i],
            };
        }
        v.mixer.master_meter = self.meters[2];
        v
    }

    fn send(&mut self, cmd: Command) {
        if self.handle.send(cmd).is_err() {
            self.message = "Audio engine is not keeping up (command queue full)".into();
        }
    }
}

fn mode_name(m: WaveformMode) -> &'static str {
    match m {
        WaveformMode::ThreeBand => "3-Band",
        WaveformMode::Rgb => "RGB",
        WaveformMode::Blue => "Blue",
    }
}

fn deck_letter(d: DeckId) -> char {
    match d {
        DeckId::A => 'A',
        DeckId::B => 'B',
    }
}
