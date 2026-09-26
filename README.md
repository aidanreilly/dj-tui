# dj-tui

A two-deck DJ console for the Linux terminal. Keyboard first, MIDI supported. GPL-3.0-or-later.

## Status

M0 and M1 are done: dj-tui plays real files through JACK.

- `engine`: deck transport, CDJ-style main cue with hold-to-preview, eight hot cues, variable
  rate, two-channel mixer with three crossfader curves and a pre-fader headphone cue bus.
  The UI talks to it over a lock-free command ring; state comes back as atomics; replaced
  tracks are freed on the UI thread. A test with a disabled allocator proves `process` never
  allocates or frees.
- `loader`: symphonia decoding (WAV, FLAC, MP3, AAC/M4A, OGG Vorbis, AIFF), stereo conversion,
  rubato resampling to the session rate with delay compensation, overview envelope, and a
  background loader thread.
- `backend`: a JACK client with `master_L/R` and `cue_L/R` ports, auto or explicit routing
  to physical outputs, xrun counting. Works with PipeWire through pipewire-jack.
- `input`, `tui`: the focus-based keymap and the stacked deck layout from the spec.
- root crate: config, CLI, and the app loop. Without a JACK server it falls back to a silent
  clock and says so on the status line.

Next up is M2: the second half of the mixer (isolator EQ, filter, trim, meters) and split-mono
routing for stereo-only cards.

Opus isn't supported: symphonia has no Opus decoder yet.

## Build and run

```sh
sudo apt install libjack-jackd2-dev libasound2-dev pipewire-jack   # or jackd2
cargo run -- ~/Music/one.flac ~/Music/two.mp3
cargo run -- --demo            # click tracks at 124 and 126 BPM
cargo run -- --no-audio x.wav  # no sound server needed
```

Under PipeWire, run it through `pw-jack` if your distro doesn't route JACK clients to PipeWire
automatically: `pw-jack cargo run -- track.flac`.

Routing lives in `~/.config/dj-tui/config.toml`:

```toml
[audio]
routing = "explicit"            # "auto" (default), "off" or "explicit"
master_ports = ["system:playback_1", "system:playback_2"]
cue_ports    = ["system:playback_3", "system:playback_4"]
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

# JACK integration test, against a throwaway dummy server:
jackd -n test -d dummy -r 48000 -p 256 -P 4 -C 0 &
JACK_DEFAULT_SERVER=test DJ_TUI_JACK_TESTS=1 cargo test -p backend --test jack_dummy
```

## Layout

```
crates/engine   deck, mixer, track buffer, realtime command channel (no I/O)
crates/loader   decoding, resampling, envelope, background loader thread
crates/backend  JACK client, port routing, planar output
crates/input    Action enum and keymap; MIDI mapping arrives in M9
crates/tui      layout, widgets, waveform rasteriser
src/            config, action application, view building, main loop
```

See `docs/spec.md` for the full specification and plan.
