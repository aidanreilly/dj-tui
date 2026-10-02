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

Every tag publishes an x86_64 Linux tarball on the
[releases page](https://github.com/aidanreilly/dj-tui/releases). Download the latest, then:

```sh
tar xzf dj-tui-*-x86_64-linux.tar.gz
cd dj-tui-*-x86_64-linux
install -Dm755 dj-tui ~/.local/bin/dj-tui   # or /usr/local/bin with sudo
```

It needs glibc 2.35 or newer and `libasound2`, which any desktop that plays audio already has.
JACK is loaded at runtime rather than linked, so the same binary serves whether or not a JACK
server is running. The tarball carries `config.toml.example` beside the binary.

Run dj-tui directly in a modern terminal emulator such as Ghostty, kitty, foot, WezTerm or a
recent Alacritty. Multiplexers are not supported.

Building from source, checksum verification and realtime scheduling:
[docs/install.md](docs/install.md).

## Run

```sh
dj-tui ~/Music/one.flac ~/Music/two.mp3   # deck A, deck B
dj-tui --demo                             # click tracks at 124 and 126 BPM
dj-tui --midi-learn                       # print what a controller sends
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

- [Installing](docs/install.md): The release tarball, building from source, putting the binary
  on your PATH, terminal requirements, realtime audio.
- [Controls](docs/controls.md): The full key reference for mix mode and browser mode.
- [The browser](docs/browser.md): Library folders, tempo and key filtering, typed filters,
  Discogs genre lookup.
- [Waveforms](docs/waveforms.md): The three colour modes and pixel rendering.
- [Configuration](docs/configuration.md): The config file, MIDI mappings, track sidecars, logs.

## Building from source

Distribution packages, the build itself and `cargo test --workspace` are in
[docs/install.md](docs/install.md#from-source).
