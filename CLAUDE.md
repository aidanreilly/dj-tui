# Working on dj-tui

Spec and milestone plan: `docs/spec.md`. M0 and M1 are done; M2 (mixer EQ, filter, trim,
meters, split-mono routing) is next.

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

`crates/wave` owns the `WavePoint` type, the biquad filter bank that produces low, mid and
high RMS per bucket at 20 points a second, and the per-track analysis cache under
`$XDG_CACHE_HOME/dj-tui/analysis`. Bump `cache::ANALYSIS_VERSION` whenever filter corners,
normalisation or `POINTS_PER_SECOND` change. Analysis runs before resampling, so one cache
file serves every session rate. Band display gains live in `tui::pixel::BAND_GAIN`, so
retuning the look never invalidates a cache file. `W` cycles `3band`, `rgb` and `blue`.

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

