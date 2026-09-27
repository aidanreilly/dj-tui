# Working on dj-tui

Spec and milestone plan: `docs/spec.md`. M0 through M3 are done. M4 has tempo and key
detection in `crates/analysis`, running on load. From M5 there are loops, beat jump and quantize, and
cues persist through the track sidecar rather than the SQLite store the spec describes. Still
missing: loops in the sidecar, M4's GiantSteps evaluation script, and M6 onwards.

## Rules

- Test first, always. Write the failing test, run it and see it fail, then write the code.
  Commit per feature with red and green in the same commit or as consecutive commits.
- Behaviour tests go in each crate's `tests/` and use only the public API.
- `engine` does no I/O and never allocates in `EngineProcessor::process`.
  `crates/engine/tests/no_alloc.rs` enforces this; extend it when adding commands or DSP.
- Old tracks leave the audio thread through the garbage ring, never by drop.
- The UI owns absolute values of continuous controls (`ControlState` in `src/apply.rs`) and
  sends absolute `Set*` commands.
- User-facing text follows the style of the existing messages: plain, specific, no jargon.

## Waveform rendering

`tui::pixel` rasterises the overview into RGBA images and `tui::Graphics` sends them through
the kitty graphics protocol when `detect_graphics()` finds it (Ghostty, kitty, WezTerm). Each
deck keeps a fixed kitty image id and only resends when the playhead reaches a new pixel
column. The glyph renderer in `tui::waveform` stays as the fallback and always draws first.
`[ui] graphics = "off"` forces glyphs.

The loader produces a per-band envelope alongside the peak envelope, and both renderers share
the colour rules that turn those bands into 3-Band, RGB or Blue. Downsampling averages bands
the same way it averages peaks, and a cell takes its colour from the row centres it covers.
`w` cycles the modes, and `[ui] waveform_mode` sets the one the app starts in.

## Loops

Loop lengths are counted in beats, so `ControlState` carries each deck's beat grid in frames
and the UI sends absolute frame positions. The engine deck wraps only on the step that crosses
the loop end, which is what lets a hot cue or a seek out of a loop keep playing. Quantize is a
UI-side snap of loop in points and jumps to the nearest beat, not something the engine knows
about. Both renderers draw the loop green: a tint plus edge lines in `tui::pixel`, a lit
background and brackets in `tui::deck`. `i` marks a loop in point and `I` closes the loop at
the playhead, which also sets the length the halve and double keys work from.

## Track sidecar

Analysis, the overview waveform and cues live in `<file name>.dj-tui.json` beside the audio,
written by `loader::sidecar`, so a library keeps its data when it moves between machines. An
`AudioId` of file size plus an FNV-1a hash of the first megabyte tells the loader when the
audio changed and the stored data no longer applies. Bump `SIDECAR_VERSION` when the meaning
of a stored field changes. Cue edits are saved from a background thread, never from the audio
thread.

## Commands

```sh
cargo test --workspace
cargo run -- --demo
cargo run -- --no-audio some.flac
# JACK integration test:
jackd -n test -d dummy -r 48000 -p 256 -P 4 -C 0 &
JACK_DEFAULT_SERVER=test DJ_TUI_JACK_TESTS=1 cargo test -p backend --test jack_dummy
```

## Toolchain

MSRV is 1.88, the minimum for ratatui 0.30.2 and ratatui-image 11.1 (kitty compression).
The workspace uses `resolver = "3"`, so `cargo update` only picks crates that build on 1.88.

