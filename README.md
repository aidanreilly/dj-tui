# dj-tui

A two-deck DJ console for the Linux terminal. Keyboard first, MIDI supported. GPL-3.0-or-later.

## Status

M0 (skeleton) is done, with parts of M1, M2 and M3 already in place:

- `engine`: deck transport, CDJ-style main cue with hold-to-preview, eight hot cues, variable
  rate with interpolation, two-channel mixer with three crossfader curves, channel faders and a
  pre-fader headphone cue bus. Allocation free in `process`.
- `input`: the focus-based keymap from spec 3.8, including `Tab` focus, the backtick
  other-deck prefix, `g`+digit seek and cue release tracking.
- `tui`: stacked deck layout with the mixer strip collapsing below 110 columns, braille
  waveform rasteriser mirrored around the centre line, deck panels, crossterm key conversion.
- root crate: config loading and validation, action application, a silent `NullClock` that
  drives the engine until the JACK backend exists, and a `--demo` mode with click tracks.

Next up is M1: symphonia decoding, rubato resampling and the JACK backend.

## Build and run

```sh
sudo apt install libasound2-dev libjack-jackd2-dev   # needed from M1 onward
cargo run -- --demo
```

Rust 1.82 or newer. `Cargo.lock` pins a few transitive crates to versions that still build on 1.82.

On terminals that support the kitty keyboard protocol (kitty, foot, WezTerm, recent Alacritty),
holding `c` previews from the cue point. Elsewhere cue works as a press.
Startup can pause for up to two seconds on terminals that don't answer the protocol query.

## Development: tests first

Every behaviour lands as a failing test before the code that makes it pass. The commit history
follows red, green, refactor per feature. Conventions:

- Behaviour tests live in each crate's `tests/` directory and use only the public API.
- The engine never touches I/O, so DSP is tested by rendering into buffers and asserting on samples.
- TUI widgets are tested by rendering into ratatui's `TestBackend` and asserting on cells,
  including style modifiers.
- Anything that talks to hardware sits behind a trait with an in-memory implementation for tests.

```sh
cargo test --workspace
```

## Layout

```
crates/engine   deck, mixer, track buffer (real-time safe, no I/O)
crates/input    Action enum and keymap; MIDI mapping arrives in M9
crates/tui      layout, widgets, waveform rasteriser
src/            config, action application, view building, main loop
```

See `docs/spec.md` for the full specification and plan.
