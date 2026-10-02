# dj-tui

A two-deck DJ console for the Linux terminal.

Two decks with CDJ-style cueing, hot cues, loops and key lock. A three-band isolator mixer
with a master filter. Tempo and key detection that runs as a track loads, and colour waveforms
drawn as real pixels where the terminal allows it.

Moving the tempo takes the pitch with it, as a turntable does. `k` locks the key instead,
holding the pitch where it is while the fader moves, through a WSOLA time stretcher that
leaves the playhead alone so loops and cues behave the same either way. Keep the pull under
about 10 %. The fader opens at ±8 % for that reason, and the wider ranges in
`[deck] tempo_range` are there for when you need the reach and will take the grain artefacts
that come with it.

Supports WAV, FLAC, MP3, AAC/M4A, OGG Vorbis and AIFF files.

<img width="1433" height="841" alt="image" src="https://github.com/user-attachments/assets/e222814b-a197-4ac1-a4b2-d9729ccd2063" />

## Install

Grab a tarball from the [releases page](https://github.com/aidanreilly/dj-tui/releases), or
build it:

```sh
cargo build --release
```

Building needs Rust 1.90 or newer and the JACK and ALSA development packages. Run dj-tui
directly in a modern terminal emulator such as Ghostty, kitty, foot, WezTerm or a recent
Alacritty. Multiplexers are not supported.

Full steps, per-distribution packages and realtime scheduling: [docs/install.md](docs/install.md).

## Run

```sh
cargo run -- ~/Music/one.flac ~/Music/two.mp3   # deck A, deck B
cargo run -- --demo                             # click tracks at 124 and 126 BPM
cargo run -- --midi-learn                       # print what a controller sends
```

## Quickstart

Enough to mix two tracks. `?` shows the key list while running.

| Key | What it does |
| --- | --- |
| `Tab` | Switch focused deck. Everything below acts on that one. |
| `Space` | Play or pause. |
| `c` | Cue. Hold to preview from the cue point, release to snap back. |
| `1`–`8` | Hot cue: jump to it, or set it where the pad is empty. |
| `s` | Sync. First press matches the other deck's tempo, second lines the beats up. |
| `,` / `.` | Pitch down and up. `<` / `>` nudge the playhead instead. |
| `k` | Key lock. |
| `l` | Four-beat loop on and off. |
| `←` / `→` | Crossfader. `Shift` takes it hard to that end, `Alt` fades it there. |
| `↑` / `↓` | The focused deck's channel fader, with the same `Shift` and `Alt` behaviour. |
| `t` / `y` / `u` | Kill the lows, mids, highs on deck A, and with `Shift` on deck B. |
| `o` / `O` | Master filter toward low-pass and high-pass. `v` centres it. |
| `m` | Headphone cue on the focused channel. `h` / `H` set the headphone mix. |
| `b` or `/` | Open the browser. `Enter` loads the selected track, `Esc` goes back. |
| `Ctrl+Q` | Quit. |

Every key, both keyboard layers and the key map images: [docs/controls.md](docs/controls.md).

## Documentation

- [Installing](docs/install.md): Binaries, building from source, terminal requirements,
  realtime audio.
- [Controls](docs/controls.md): The full key reference for mix mode and browser mode.
- [The browser](docs/browser.md): Library folders, tempo and key filtering, typed filters,
  Discogs genre lookup.
- [Waveforms](docs/waveforms.md): The three colour modes and pixel rendering.
- [Configuration](docs/configuration.md): The config file, MIDI mappings, track sidecars, logs.

## Building and testing

```sh
cargo test --workspace
```
