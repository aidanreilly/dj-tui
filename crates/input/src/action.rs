use engine::DeckId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Band {
    High,
    Mid,
    Low,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Focus(DeckId),
    PlayPause(DeckId),
    CuePress(DeckId),
    CueRelease(DeckId),
    HotCue(DeckId, usize),
    ClearHotCue(DeckId, usize),
    /// Direction and whether this is a fine step.
    Tempo(DeckId, Dir, bool),
    /// Move both decks' tempo by the same proportion, so a matched pair stays matched.
    /// Direction and whether this is a fine step.
    GlobalTempo(Dir, bool),
    Nudge(DeckId, Dir),
    Sync(DeckId),
    Quantize(DeckId),
    KeyLock(DeckId),
    LoopToggle(DeckId),
    /// Set the loop in point at the playhead, leaving the loop open.
    LoopIn(DeckId),
    /// Close a loop that `LoopIn` opened, from the in point to the playhead.
    LoopOut(DeckId),
    LoopHalve(DeckId),
    LoopDouble(DeckId),
    BeatJump(DeckId, Dir),
    SeekTenth(DeckId, u8),
    Trim(DeckId, Dir),
    Eq(DeckId, Band, Dir),
    EqKill(DeckId, Band),
    /// The master filter: toward low-pass or toward high-pass.
    Filter(Dir),
    Fader(DeckId, Dir),
    /// Send the fader hard to an end.
    FaderEnd(DeckId, Dir),
    /// Put the master filter back at neutral.
    FilterCentre,
    CrossfaderCentre,
    /// Fade the crossfader to an end over the fade length.
    CrossfaderFade(Dir),
    CrossfaderFadeCentre,
    /// Fade a channel fader to full or to silence.
    FaderFade(DeckId, Dir),
    /// Sweep the master filter to an end.
    FilterSweep(Dir),
    FilterSweepCentre,
    /// Halve or double the shared fade length.
    FadeLength(Dir),
    /// Stop every running fade where it stands.
    CancelFades,
    HeadphoneCue(DeckId),
    /// Blend the headphones between the cue bus and the master.
    CueMix(Dir),
    /// Direction and whether to snap to the end.
    Crossfader(Dir, bool),
    BrowserMove(Dir),
    Load(DeckId),
    Search,
    BrowserFullscreen,
    /// Cycle the column the browser is sorted by, or with `true` turn the order around.
    BrowserSort(bool),
    /// Cycle the BPM window: off, within 3 % of the playing deck, within 6 %.
    BrowserFilterBpm,
    /// Show only the keys that would mix with the playing deck's.
    BrowserFilterKey,
    /// Cycle through the genres the library holds, and back to off.
    BrowserFilterGenre,
    /// Analyse every track in the browser that has no sidecar yet.
    AnalyseLibrary,
    /// Give the browser the keyboard.
    BrowserEnter,
    /// Hand the keyboard back to mix mode.
    BrowserLeave,
    /// A character typed into the browser's query.
    BrowserType(char),
    BrowserBackspace,
    /// Empty the query, leaving the mode alone.
    BrowserClear,
    DeviceMove(Dir),
    DeviceChoose,
    DeviceClose,
    /// Open the audio device chooser.
    Devices,
    CycleWaveformMode,
    Help,
    Quit,
}
