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
    FxToggle(DeckId),
    FxNext(DeckId),
    FxWet(DeckId, Dir),
    /// One of the effect's two knobs, by index.
    FxParam(DeckId, usize, Dir),
    SeekTenth(DeckId, u8),
    Trim(DeckId, Dir),
    Eq(DeckId, Band, Dir),
    EqKill(DeckId, Band),
    Filter(DeckId, Dir),
    Fader(DeckId, Dir),
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
    CycleWaveformMode,
    Help,
    Quit,
}
