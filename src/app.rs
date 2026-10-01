//! UI-side application state. Owns the engine handle, keymap, control values, track
//! metadata and the background loader. Everything here runs on the UI thread.

use crate::apply::{apply_many, ControlState, Controls};
use crate::automation::{Automation, Curve};
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
    /// The key list is up.
    help: bool,
    /// Events worth keeping: failures and warnings, not every message on screen.
    log: Vec<String>,
    browser: crate::browser::Browser,
    /// Files waiting for background analysis, and how many the run started with.
    analysing: std::collections::VecDeque<PathBuf>,
    analysis_total: usize,
    /// The audio devices offered, and which is in use.
    devices: Vec<(String, String)>,
    current_device: String,
    device_note: String,
    /// Where the chooser is sitting, while it is open.
    device_selected: Option<usize>,
    /// A device the user picked, for the caller to act on.
    chosen_device: Option<String>,
    /// Fades running on their own, and the length they run for.
    automation: Automation,
    /// Values a fade produced this frame, reused so the tick allocates nothing.
    fade_values: Vec<(midi::Control, f32)>,
    /// When the last frame was, for the fade tick's delta.
    last_frame: std::time::Instant,
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
            controls: Controls::new(config.deck.tempo_range, sample_rate),
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
            help: false,
            log: Vec::new(),
            browser: crate::browser::Browser::new(),
            analysing: std::collections::VecDeque::new(),
            analysis_total: 0,
            devices: Vec::new(),
            current_device: String::new(),
            device_note: String::new(),
            device_selected: None,
            chosen_device: None,
            automation: Automation::new(config.mixer.fade_beats),
            fade_values: Vec::with_capacity(8),
            last_frame: std::time::Instant::now(),
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
        // A fade aimed at a track being replaced is aimed at nothing.
        self.automation.cancel_deck(deck);
        self.metas[deck.index()].loading = true;
        self.message = format!("Loading {} on deck {}…", path.display(), deck_letter(deck));
        self.loader.request(deck, path);
    }

    /// Put an already decoded track on a deck (demo mode, tests).
    pub fn load_track(&mut self, deck: DeckId, track: engine::Track, meta: DeckMeta) {
        let i = deck.index();
        let rate = self.sample_rate as f64;
        // The loop and jump keys work in beats, so they need the grid in frames.
        self.state.beat_frames[i] = meta.grid.map(|g| g.beat_secs() * rate);
        self.state.first_beat_frames[i] = meta.grid.map_or(0.0, |g| g.first_beat_secs * rate);
        self.state.loop_in[i] = None;
        self.metas[i] = DeckMeta {
            loading: false,
            ..meta
        };
        self.send(Command::Load(deck, Arc::new(track)));
    }

    /// Call once per UI frame: frees replaced tracks and picks up finished loads.
    pub fn tick(&mut self) {
        let dt = self.last_frame.elapsed().as_secs_f64();
        self.last_frame = std::time::Instant::now();
        self.tick_fades(dt);
        self.handle.collect_garbage();
        self.save_changed_cues();
        while let Some(note) = self.loader.try_recv_note() {
            self.log.push(note.clone());
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
        while let Some(done) = self.loader.try_recv_analysed() {
            self.analysing.pop_front();
            match done.error {
                Some(e) => {
                    let line = format!("Could not analyse {}: {e}", done.path.display());
                    self.log.push(line.clone());
                    self.message = line;
                }
                None => self.browser.refresh(&done.path),
            }
            if self.analysing.is_empty() {
                if self.analysis_total > 0 {
                    self.message = format!("Analysed {} tracks", self.analysis_total);
                }
                self.analysis_total = 0;
                self.browser.set_note(None);
            } else {
                self.next_analysis();
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
                        Some(note) => {
                            let line = format!(
                                "Loaded {title} on deck {}, but {note}",
                                deck_letter(done.deck)
                            );
                            self.log.push(line.clone());
                            line
                        }
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
                    self.log.push(self.message.clone());
                }
            }
        }
    }

    /// The file on each deck, deck A first, for writing the session out. It comes from the
    /// same record the cue saving uses, so it names the track the engine is actually playing
    /// rather than the last one asked for.
    pub fn deck_paths(&self) -> [Option<PathBuf>; 2] {
        [0, 1].map(|i| self.persisted[i].as_ref().map(|p| p.path.clone()))
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
        if let Some((start, end)) = cues.loop_secs {
            self.send(Command::SetLoop(deck, Some((start * rate, end * rate))));
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
                loop_secs: d.loop_span.map(|(start, end)| (start / rate, end / rate)),
            };
            if now != p.cues {
                self.loader.save_cues(p.path.clone(), now.clone());
                p.cues = now;
            }
        }
    }

    /// Report what the loop, loop length and quantize keys did. Their effect is otherwise
    /// invisible until the next frame, and a loop key on an unanalysed track does nothing.
    fn note_loop_keys(&mut self, action: Action, cmd: Option<&Command>) {
        let (Action::LoopToggle(d)
        | Action::LoopHalve(d)
        | Action::LoopDouble(d)
        | Action::LoopIn(d)
        | Action::LoopOut(d)
        | Action::Quantize(d)) = action
        else {
            return;
        };
        let letter = deck_letter(d);
        let beats = beats_label(self.state.loop_beats[d.index()]);
        self.message = match (action, cmd) {
            (Action::Quantize(_), _) => {
                let on = if self.state.quantize[d.index()] {
                    "on"
                } else {
                    "off"
                };
                format!("Quantize {on} on deck {letter}")
            }
            (Action::LoopIn(_), _) => {
                let secs = self.state.loop_in[d.index()].unwrap_or(0.0) / self.sample_rate as f64;
                format!(
                    "Loop in at {} on deck {letter}, press I to close it",
                    mmss(secs)
                )
            }
            (Action::LoopOut(_), None) => {
                if self.state.loop_in[d.index()].is_some() {
                    format!("Deck {letter}: the loop out point is behind the loop in point")
                } else {
                    format!("Deck {letter} has no loop in point yet, press i first")
                }
            }
            (Action::LoopOut(_), _) => format!("Loop: {beats} on deck {letter}"),
            (_, None) => format!("Deck {letter} has no beat grid, so loops have no length"),
            (Action::LoopToggle(_), Some(Command::SetLoop(_, None))) => {
                format!("Loop off on deck {letter}")
            }
            (Action::LoopToggle(_), _) => format!("Loop: {beats} on deck {letter}"),
            _ => format!("Loop length: {beats} on deck {letter}"),
        };
    }

    /// Report a global tempo ride in BPM, since that is what the person is thinking in even
    /// though the step is a proportion. A refused move says why rather than going quiet.
    fn note_global_tempo(&mut self, action: Action, moved: bool) {
        let Action::GlobalTempo(..) = action else {
            return;
        };
        if !moved {
            self.message =
                "Both decks cannot go further without one leaving its tempo range".into();
            return;
        }
        let bpm = |i: usize| {
            self.metas[i]
                .bpm
                .map(|b| format!("{:.2}", b * self.state.rates[i]))
        };
        self.message = match (bpm(0), bpm(1)) {
            (Some(a), Some(b)) => format!("Tempo: A {a} BPM, B {b} BPM"),
            (Some(a), None) => format!("Tempo: A {a} BPM"),
            (None, Some(b)) => format!("Tempo: B {b} BPM"),
            // Nothing analysed, so say it as the faders read.
            (None, None) => format!(
                "Tempo: {:+.1}% on both decks",
                (self.state.rates[0] - 1.0) * 100.0
            ),
        };
    }

    /// Report the headphone blend, which only the person wearing them can hear.
    fn note_cue_mix(&mut self, action: Action) {
        let Action::CueMix(_) = action else { return };
        let master = (self.state.cue_mix * 100.0).round() as u32;
        self.message = format!("Headphones: {}% cue, {master}% master", 100 - master);
    }

    /// Report the key lock toggle, which has no effect at all until the tempo fader moves.
    fn note_key_lock(&mut self, action: Action) {
        let Action::KeyLock(d) = action else { return };
        let on = if self.state.key_lock[d.index()] {
            "on"
        } else {
            "off"
        };
        self.message = format!("Key lock {on} on deck {}", deck_letter(d));
    }

    /// Report what the sync key did. Matching tempo and lining the beats up are separate
    /// presses, and a deck with no grid can do neither.
    fn note_sync_key(&mut self, action: Action, cmd: Option<&Command>) {
        let Action::Sync(d) = action else { return };
        let letter = deck_letter(d);
        let snap = self.handle.snapshot();
        let other = &snap.decks[1 - d.index()];
        self.message = match cmd {
            // With nothing running to follow, this deck keeps its own time.
            None if other.track_frames > 0 && !other.playing => {
                format!("Deck {letter} is the only one running, so it sets the time")
            }
            None => format!("Deck {letter} needs a beat grid on both decks to sync"),
            Some(Command::SetRate(_, _)) => {
                let bpm = self.metas[d.index()]
                    .bpm
                    .map(|b| b * self.state.rates[d.index()]);
                match bpm {
                    Some(bpm) => format!("Sync: deck {letter} at {bpm:.2} BPM"),
                    None => format!("Sync: deck {letter} matched"),
                }
            }
            Some(_) => format!("Sync: deck {letter} beats lined up"),
        };
    }

    /// Handle one key event. Returns true when the user asked to quit.
    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        let Some(action) = self.keymap.handle(key) else {
            return false;
        };
        self.on_action(action)
    }

    /// Offer `devices` in the chooser, with `current` the one in use and `note` what it is
    /// doing. The caller owns the audio, so it owns this list.
    pub fn set_devices(&mut self, devices: Vec<(String, String)>, current: String, note: String) {
        self.devices = devices;
        self.current_device = current;
        self.device_note = note;
    }

    /// A device the user picked since the last call.
    pub fn chosen_device(&mut self) -> Option<String> {
        self.chosen_device.take()
    }

    fn move_device(&mut self, by: isize) {
        let Some(selected) = self.device_selected else {
            return;
        };
        if self.devices.is_empty() {
            return;
        }
        let last = self.devices.len() - 1;
        self.device_selected = Some(match by {
            b if b < 0 => selected.saturating_sub(1),
            _ => (selected + 1).min(last),
        });
    }

    fn choose_device(&mut self) {
        let Some(selected) = self.device_selected.take() else {
            return;
        };
        let Some((name, _)) = self.devices.get(selected) else {
            return;
        };
        self.message = format!("Audio device: {name}");
        self.chosen_device = Some(name.clone());
    }

    /// Which deck the browser measures against: whichever is playing, deck A first. Both
    /// the key highlighting and the filters follow this one rule.
    fn playing_deck(&self) -> Option<usize> {
        let snapshot = self.handle.snapshot();
        [0, 1].into_iter().find(|&i| snapshot.decks[i].playing)
    }

    /// The key of whichever deck is playing, or `None` with nothing playing and with a track
    /// the detector could not read.
    fn playing_key(&self) -> Option<analysis::key::Key> {
        self.playing_deck()
            .and_then(|i| self.metas[i].key.as_deref())
            .and_then(analysis::key::Key::from_camelot)
    }

    /// The tempo of whichever deck is playing, under the same rule.
    fn playing_bpm(&self) -> Option<f64> {
        self.playing_deck().and_then(|i| self.metas[i].bpm)
    }

    /// Analyse everything in the browser that has no sidecar yet, one file at a time so the
    /// machine stays usable while it runs.
    fn start_analysis(&mut self) {
        if !self.analysing.is_empty() {
            self.analysing.clear();
            self.analysis_total = 0;
            self.browser.set_note(None);
            self.message = "Analysis stopped".into();
            return;
        }
        self.analysing = self.browser.unanalysed().into();
        self.analysis_total = self.analysing.len();
        match self.analysis_total {
            0 => self.message = "Every track is already analysed".into(),
            n => {
                self.message = format!("Analysing {n} tracks. A again stops it.");
                self.next_analysis();
            }
        }
    }

    /// Send the next file to the loader and say how far along the run is.
    fn next_analysis(&mut self) {
        let Some(path) = self.analysing.front().cloned() else {
            self.browser.set_note(None);
            self.analysis_total = 0;
            return;
        };
        let done = self.analysis_total - self.analysing.len() + 1;
        self.browser
            .set_note(Some(format!("analysing {done} of {}", self.analysis_total)));
        self.loader.analyse(path);
    }

    /// Read `roots` into the browser.
    pub fn scan_library(&mut self, roots: &[PathBuf]) {
        self.browser.scan(roots);
        let count = self.browser.entries().len();
        self.message = match count {
            0 => "No music found. Set [library] folders in the config.".into(),
            n => format!("Library: {n} tracks"),
        };
    }

    /// Put a control where a MIDI fader or knob says it is.
    pub fn set_control(&mut self, control: midi::Control, value: f32) {
        // A hand on the hardware always wins.
        self.automation.cancel(control);
        if let Some(cmd) =
            crate::apply::set_control(&mut self.state, &self.controls, control, value)
        {
            self.send(cmd);
        }
    }

    pub fn key_lock(&self, deck: DeckId) -> bool {
        self.state.key_lock[deck.index()]
    }

    pub fn quantize(&self, deck: DeckId) -> bool {
        self.state.quantize[deck.index()]
    }

    /// True when this deck is playing at the same beat length as the other one, which is what
    /// a sync light on a controller follows.
    pub fn tempo_matches_other_deck(&self, deck: DeckId) -> bool {
        let (i, other) = (deck.index(), 1 - deck.index());
        let (Some(mine), Some(theirs)) = (self.state.beat_frames[i], self.state.beat_frames[other])
        else {
            return false;
        };
        let played = |beat: f64, rate: f64| beat / rate;
        let a = played(mine, self.state.rates[i]);
        let b = played(theirs, self.state.rates[other]);
        (a - b).abs() < a * 1e-3
    }

    /// Where the UI holds a control, from 0 to 1, for a controller's soft takeover.
    pub fn control_value(&self, control: midi::Control) -> f32 {
        crate::apply::control_value(&self.state, &self.controls, control)
    }

    /// Handle one action, whichever input produced it. Returns true when the user asked to quit.
    pub fn on_action(&mut self, action: Action) -> bool {
        // Any key closes the help overlay; `?` is the only one that opens it.
        let was_help = std::mem::take(&mut self.help);
        match action {
            Action::Help => {
                self.help = !was_help;
                return false;
            }
            Action::Quit => return true,
            Action::CycleWaveformMode => {
                self.waveform_mode = self.waveform_mode.next();
                self.message = format!("Waveform: {}", mode_name(self.waveform_mode));
            }
            Action::Load(deck) => match self.browser.selected_path() {
                Some(path) => {
                    let path = path.to_path_buf();
                    self.load_path(deck, path);
                }
                None => self.message = "Nothing selected in the browser".into(),
            },
            Action::BrowserMove(dir) => self.browser.move_selection(dir),
            Action::BrowserSort(reverse) => self.browser.sort_by(reverse),
            Action::BrowserFilterBpm => {
                let playing = self.playing_bpm();
                self.message = self.browser.cycle_bpm_filter(playing);
            }
            Action::BrowserFilterKey => {
                let playing = self.playing_key();
                self.message = self.browser.toggle_key_filter(playing);
            }
            Action::BrowserFilterGenre => self.message = self.browser.cycle_genre_filter(),
            Action::BrowserFullscreen => self.browser.fullscreen = !self.browser.fullscreen,
            Action::Search => {
                self.browser.clear_query();
                self.keymap.set_mode(input::Mode::Browser);
            }
            // `b` is a decision to go and look through the library, so the list gets the
            // screen. `/` is the way in that leaves the decks visible, and Alt+f moves
            // between the two without leaving the browser.
            Action::BrowserEnter => {
                self.browser.fullscreen = true;
                self.keymap.set_mode(input::Mode::Browser);
            }
            Action::BrowserLeave => {
                // Full screen is a browser-mode affordance; leaving it behind would strand
                // the person in mix mode with no mixer on screen and no key to bring it back.
                self.browser.fullscreen = false;
                self.keymap.set_mode(input::Mode::Mix);
            }
            Action::BrowserType(c) => self.browser.type_char(c),
            Action::BrowserBackspace => self.browser.backspace(),
            Action::BrowserClear => self.browser.clear_query(),
            Action::DeviceMove(dir) => self.move_device(match dir {
                input::Dir::Up => -1,
                input::Dir::Down => 1,
            }),
            Action::DeviceChoose => {
                self.choose_device();
                self.keymap.set_mode(input::Mode::Mix);
            }
            Action::DeviceClose => {
                self.device_selected = None;
                self.keymap.set_mode(input::Mode::Mix);
            }
            Action::AnalyseLibrary => self.start_analysis(),
            Action::CrossfaderFade(dir) => {
                let to = end_of_travel(dir);
                let focused = self.keymap.focused();
                self.start_fade(midi::Control::Crossfader, to, Curve::Position, focused);
            }
            Action::CrossfaderFadeCentre => {
                let focused = self.keymap.focused();
                self.start_fade(midi::Control::Crossfader, 0.5, Curve::Position, focused);
            }
            Action::FaderFade(d, dir) => {
                self.start_fade(
                    midi::Control::Fader(d),
                    end_of_travel(dir),
                    Curve::Decibel,
                    d,
                );
            }
            Action::FilterSweep(dir) => {
                self.start_fade(
                    midi::Control::Filter,
                    end_of_travel(dir),
                    Curve::Position,
                    self.keymap.focused(),
                );
            }
            Action::FilterSweepCentre => {
                self.start_fade(
                    midi::Control::Filter,
                    0.5,
                    Curve::Position,
                    self.keymap.focused(),
                );
            }
            Action::FadeLength(dir) => {
                self.automation.scale_length(dir);
                self.message = format!("Fade length: {} beats", self.automation.fade_beats());
            }
            Action::CancelFades => {
                // Silent when there is nothing to stop, so Esc is safe to lean on.
                if self.automation.running() > 0 {
                    self.automation.cancel_all();
                    self.message = "Fades stopped".into();
                }
            }
            Action::Devices => {
                // Open on the device in use, so Enter on it changes nothing.
                let current = self
                    .devices
                    .iter()
                    .position(|(name, _)| *name == self.current_device);
                self.device_selected = Some(current.unwrap_or(0));
                self.keymap.set_mode(input::Mode::Devices);
            }
            _ => {
                if let Some(control) = manual_control(action) {
                    self.automation.cancel(control);
                }
                let snap = self.handle.snapshot();
                // Riding the global tempo moves two decks, so both commands go in the same
                // frame and land in the same audio callback.
                let cmds = apply_many(&mut self.state, &self.controls, &snap, action);
                self.note_loop_keys(action, cmds.first());
                self.note_sync_key(action, cmds.first());
                self.note_key_lock(action);
                self.note_cue_mix(action);
                self.note_global_tempo(action, !cmds.is_empty());
                for cmd in cmds {
                    self.send(cmd);
                }
            }
        }
        false
    }

    /// Advance running fades by `dt` seconds and send what they reached. Called once a frame,
    /// and directly from tests.
    pub fn tick_fades(&mut self, dt: f64) {
        let mut values = std::mem::take(&mut self.fade_values);
        values.clear();
        self.automation.tick(dt, &mut values);
        for &(control, v) in &values {
            if let Some(cmd) =
                crate::apply::set_control(&mut self.state, &self.controls, control, v)
            {
                self.send(cmd);
            }
        }
        self.fade_values = values;
    }

    /// Start a fade on `control` toward `to`, taking its length from `deck`'s beat grid.
    /// Targets are in the 0-to-1 space `Control` uses, so the crossfader's centre is 0.5.
    fn start_fade(&mut self, control: midi::Control, to: f32, curve: Curve, deck: DeckId) {
        let i = deck.index();
        let secs = self.automation.secs_for(
            self.state.beat_frames[i],
            self.state.rates[i],
            self.sample_rate,
        );
        let from = crate::apply::control_value(&self.state, &self.controls, control);
        self.automation.start(control, from, to, secs, curve);
        if self.automation.target(control).is_some() {
            self.message = format!(
                "{} over {} beats",
                fade_label(control, to),
                self.automation.fade_beats()
            );
        }
    }

    /// Seek a loaded deck to a normalized position from 0.0 (start) to 1.0 (end).
    pub fn seek_to_fraction(&mut self, deck: DeckId, fraction: f64) {
        let frames = self.handle.snapshot().decks[deck.index()].track_frames;
        if frames == 0 {
            return;
        }
        self.send(Command::Seek(
            deck,
            frames as f64 * fraction.clamp(0.0, 1.0),
        ));
    }

    pub fn snapshot(&self) -> Snapshot {
        self.handle.snapshot()
    }

    /// Put a line in the message area, for something the app did not do itself.
    pub fn note(&mut self, text: String) {
        self.message = text;
    }

    /// Take everything worth writing to the log since the last call.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
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
        v.help = self.help;
        // Highlight against whichever deck is playing; deck A wins when both are.
        let playing = self.playing_key();
        let browsing = self.keymap.mode() == input::Mode::Browser;
        v.browser = self.browser.view(browsing, playing);
        // Say which mode holds the keyboard, since that changes what every letter does.
        if browsing {
            v.status = format!("BROWSER  {}", v.status);
        }
        v.devices = self.device_selected.map(|selected| tui::DeviceView {
            devices: self.devices.clone(),
            selected,
            current: self.current_device.clone(),
            note: self.device_note.clone(),
        });
        let elapsed = self.started.elapsed().as_secs_f64();
        for (i, d) in v.decks.iter_mut().enumerate() {
            d.waveform_mode = self.waveform_mode;
            d.quantize = self.state.quantize[i];
            d.key_lock = self.state.key_lock[i];
            d.kills = self.state.strips[i].kills;
            d.loop_in_secs = self.state.loop_in[i].map(|f| f / self.sample_rate as f64);
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
                meter: self.meters[i],
                fade_target: self.automation.target(midi::Control::Fader(if i == 0 {
                    DeckId::A
                } else {
                    DeckId::B
                })),
            };
        }
        v.mixer.master_meter = self.meters[2];
        v.mixer.focused = self.keymap.focused();
        v.mixer.fade_beats = self.automation.fade_beats();
        v.mixer.crossfader_fade_target = self
            .automation
            .target(midi::Control::Crossfader)
            .map(|t| t * 2.0 - 1.0);
        v.mixer.cue_mix = self.state.cue_mix;
        v.mixer.filter = self.state.filter;
        v
    }

    fn send(&mut self, cmd: Command) {
        if self.handle.send(cmd).is_err() {
            self.message = "Audio engine is not keeping up (command queue full)".into();
        }
    }
}

/// Where a direction points on a control's own travel, in the 0-to-1 `Control` space.
fn end_of_travel(dir: input::Dir) -> f32 {
    match dir {
        input::Dir::Up => 1.0,
        input::Dir::Down => 0.0,
    }
}

/// The control a stepped or slammed action moves, so its fade can let go of it.
fn manual_control(action: Action) -> Option<midi::Control> {
    Some(match action {
        Action::Crossfader(..) | Action::CrossfaderCentre => midi::Control::Crossfader,
        Action::Fader(d, _) | Action::FaderEnd(d, _) => midi::Control::Fader(d),
        Action::Filter(_) | Action::FilterCentre => midi::Control::Filter,
        Action::Trim(d, _) => midi::Control::Trim(d),
        Action::Tempo(d, ..) => midi::Control::Tempo(d),
        Action::CueMix(_) => midi::Control::CueMix,
        _ => return None,
    })
}

/// What a starting fade says for itself. A fade cannot be seen beginning, only heard.
fn fade_label(control: midi::Control, to: f32) -> String {
    match control {
        midi::Control::Crossfader => match to {
            t if t > 0.75 => "Crossfader to B".into(),
            t if t < 0.25 => "Crossfader to A".into(),
            _ => "Crossfader to the middle".into(),
        },
        midi::Control::Fader(d) => {
            let way = if to > 0.5 { "up" } else { "down" };
            format!("Deck {} fader {way}", deck_letter(d))
        }
        midi::Control::Filter => {
            let way = match to {
                t if t > 0.75 => "to high-pass",
                t if t < 0.25 => "to low-pass",
                _ => "back to the middle",
            };
            format!("Filter {way}")
        }
        _ => "Fade".into(),
    }
}

fn mode_name(m: WaveformMode) -> &'static str {
    match m {
        WaveformMode::ThreeBand => "3-Band",
        WaveformMode::Rgb => "RGB",
        WaveformMode::Blue => "Blue",
    }
}

/// Minutes and seconds, as the deck panel shows times.
fn mmss(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Loop lengths read as beats above one and as a fraction of a beat below it.
fn beats_label(beats: f64) -> String {
    if beats >= 1.0 {
        let whole = beats.round();
        if (beats - whole).abs() < 1e-9 && whole == 1.0 {
            "1 beat".into()
        } else {
            format!("{} beats", (beats * 100.0).round() / 100.0)
        }
    } else {
        format!("1/{} beat", (1.0 / beats).round())
    }
}

fn deck_letter(d: DeckId) -> char {
    match d {
        DeckId::A => 'A',
        DeckId::B => 'B',
    }
}
