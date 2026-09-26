use engine::DeckId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir { Up, Down }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band { High, Mid, Low }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Focus(DeckId),
    PlayPause(DeckId), CuePress(DeckId), CueRelease(DeckId),
    HotCue(DeckId, usize), ClearHotCue(DeckId, usize),
    /// Direction and whether this is a fine step.
    Tempo(DeckId, Dir, bool),
    Nudge(DeckId, Dir),
    Sync(DeckId), Quantize(DeckId), KeyLock(DeckId),
    LoopToggle(DeckId), LoopHalve(DeckId), LoopDouble(DeckId),
    BeatJump(DeckId, Dir),
    FxToggle(DeckId), FxNext(DeckId), FxWet(DeckId, Dir),
    SeekTenth(DeckId, u8),
    Trim(DeckId, Dir), Eq(DeckId, Band, Dir), EqKill(DeckId, Band),
    Filter(DeckId, Dir), Fader(DeckId, Dir), HeadphoneCue(DeckId),
    /// Direction and whether to snap to the end.
    Crossfader(Dir, bool),
    BrowserMove(Dir), Load(DeckId), Search, BrowserFullscreen,
    CycleWaveformMode, Help, Quit,
}
