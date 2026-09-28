# dj-tui

A two-deck DJ console for the Linux terminal.

Two decks with CDJ-style cueing, hot cues, loops and key lock, a three-band isolator mixer with a master filter, colour waveforms drawn as real pixels where the terminal allows it, and tempo and key detection that runs as a track loads.

Supports WAV, FLAC, MP3, AAC/M4A, OGG Vorbis and AIFF files.

<img width="800" alt="image" src="https://github.com/user-attachments/assets/76bcab07-5c76-42d9-a235-1938aa7b77fc" />

## Install

Fedora:

```sh
sudo dnf install cargo pipewire-jack-audio-connection-kit-devel
```

Debian and Ubuntu:

```sh
sudo apt install cargo libjack-jackd2-dev libasound2-dev pipewire-jack
```

## Terminal requirements

Run dj-tui directly in a modern terminal emulator such as Ghostty, kitty, foot, WezTerm, recent Alacritty.

Terminal multiplexers are not supported.

## Run

```sh
cargo run -- ~/Music/one.flac ~/Music/two.mp3   # deck A, deck B
cargo run -- --demo                             # click tracks at 124 and 126 BPM
cargo run -- --midi-learn                       # print what a controller sends
```

## Controls

![The dj-tui key map on a UK keyboard](images/keyboard.svg)

One row per key. `Shift +`, `Alt +` or `Ctrl +` change key travel.

| Key | What it does |
| --- | --- |
| `Space` | Play or pause the focused deck. |
| `c` | Cue. Hold to preview from the cue point, release to snap back. |
| `1`–`8` | Hot cue: jump to it, or set it where the pad is empty. `Alt` clears one. |
| `g` then `0`–`9` | Seek to that tenth of the track. |
| `-` / `+` | Ride both decks' tempo together by the same proportion, so a beatmatched pair stays matched. `Alt` gives a fine step. |
| `,` / `.` | The focused deck's own pitch, down and up, with `Alt` for a fine step. Shift (`<` / `>`) nudges the playhead instead, for matching by ear. |
| `s` | Sync. First press matches the other deck's tempo, second lines the beats up. |
| `q` | Quantize: snap loop points and beat jumps to the beat grid. |
| `k` | Key lock: hold the pitch where it is while the tempo fader moves. |
| `l` | Four-beat loop on and off. |
| `i` / `I` | Mark the loop in point. Shift closes the loop where the playhead is, which also sets the length. |
| `[` / `]` | Halve and double the loop, keeping the in point. Shift (`{` / `}`) halves and doubles the fade length instead. |
| `j` / `J` | Beat jump back by the loop length, forward with Shift. |
| `←` / `→` | Crossfader one step. Shift takes it hard to that end, `Alt` fades it there over the fade length. |
| `↑` / `↓` | The focused deck's channel fader one step, in dB. Shift goes hard to full or to zero, `Alt` fades. |
| `x` / `X` | Crossfader back to the middle. Shift fades it there instead. |
| `r` / `R` | Trim down, and up with Shift. |
| `t` / `y` / `u` | Kill the lows, mids, highs on deck A. Shift (`T` / `Y` / `U`) kills them on deck B. A press toggles, so pressing again gives the band back. |
| `o` / `O` | Master filter toward low-pass, and toward high-pass with Shift. `Alt` sweeps it there slowly. |
| `v` / `V` | Master filter back to the middle. Shift sweeps it back over the fade length. |
| `m` | Headphone cue on the focused channel. |
| `h` / `H` | Headphone mix, from the cue bus toward the master with Shift. |
| `w` | Waveform colour mode: 3-Band, RGB, Blue. Shift does the same thing. |
| `b` | The browser, full screen, keeping whatever filter is on. |
| `/` | The browser beside the decks, with the filter cleared. Shift (`?`) shows the key list instead. |
| `Enter` | Load the selected track onto the focused deck. |
| `Tab` | Switch focused deck, in every mode, which is what picks where `Enter` sends a track. |
| `Esc` | Stop every running fade. In the browser or on the device screen it returns to mix mode in one press. |
| `Backspace` | In the browser, edit the filter. |
| `Alt+s` / `Alt+S` | In the browser, change the sort column, and with Shift the direction. |
| `Alt+a` | In the browser, analyse everything in the list with no analysis yet. Press it again to stop. |
| `Alt+f` | In the browser, swap between full screen and the panel beside the decks. |
| `Ctrl+u` | In the browser, clear the filter. |
| `Ctrl+D` | The audio device screen, where `↑` / `↓` and `Enter` choose a card. |
| `Ctrl+Q` | Quit, from any mode. |
| Left click on a waveform | Seek there. |

### Transport

| Key | What it does |
| --- | --- |
| `Space` | Play or pause |
| `c` | Cue. Hold to preview from the cue point, release to snap back |
| `1`–`8` | Hot cue: jump to it, or set it if the pad is empty |
| `Alt+1`–`8` | Clear a hot cue |
| `g` then `0`–`9` | Seek to a tenth of the track |
| `-` / `+` | Ride both decks' tempo together, `Alt` for a fine step |
| `,` / `.` | The focused deck's pitch, `Alt` for a fine step |
| `<` / `>` | Nudge the playhead back and forward, for beatmatching by ear |
| `s` | Sync: first press matches the other deck's tempo, second lines the beats up |
| `k` | Key lock: hold the pitch while the tempo fader moves |
| `q` | Quantize: snap loops and jumps to the beat grid |

Sync needs something running to follow. When the other deck's track ends it stops being a
timing reference, and the deck still playing keeps its own time.

### Loops

| Key | What it does |
| --- | --- |
| `l` | Four-beat loop on and off |
| `i` / `I` | Mark the loop in point, then close the loop where the playhead is |
| `[` / `]` | Halve and double the loop, keeping the in point |
| `j` / `J` | Beat jump back and forward by the loop length |

A loop marked by hand needs no beat grid, so it works on anything. The running loop is saved
with the track and comes back the next time you load it.

### Mixer

| Key | What it does |
| --- | --- |
| `←` / `→` | Crossfader, one step |
| `Shift+←` / `Shift+→` | Crossfader hard to that end |
| `↑` / `↓` | Focused deck's channel fader, one step |
| `Shift+↑` / `Shift+↓` | Focused deck's fader hard to full or to zero |
| `x` | Crossfader to the middle |
| `t` / `y` / `u` | Kill the lows, mids, highs on deck A, and with Shift on deck B |
| `r` / `R` | Trim |
| `o` / `O` | Master filter toward low-pass and high-pass |
| `v` | Master filter back to the middle |
| `m` | Headphone cue on this channel |
| `h` / `H` | Headphone mix, from the cue bus toward the master |

### Fades

| Key | What it does |
| --- | --- |
| `Alt+←` / `Alt+→` | Crossfader fades to that end |
| `Alt+↑` / `Alt+↓` | Focused deck's fader fades to full or to silence |
| `Alt+o` / `Alt+O` | Master filter sweeps to that end |
| `X` | Crossfader fades to the middle |
| `V` | Master filter sweeps back to the middle |
| `{` / `}` | Halve and double the fade length |
| `Esc` | Stop every running fade where it stands |

### Everything else

| Key | What it does |
| --- | --- |
| `Tab` | Switch focused deck, in either mode |
| `w` | Waveform colour mode: 3-Band, RGB, Blue |
| Left click on a waveform | Seek there |
| `Enter` | Load the selected track onto the focused deck |
| `b` | Browser mode, full screen, keeping whatever filter is on |
| `/` | Browser mode beside the decks, clearing the filter |
| `Ctrl+D` | Audio device screen |
| `?` | Key list |
| `Ctrl+Q` | Quit |

In browser `Alt+s` and `Alt+S` sort, `Alt+a` analyses, `Alt+f` swaps between full screen and the panel, `Ctrl+u` clears the filter.

## The browser

Point dj-tui at your music:

```toml
[library]
folders = ["~/Music", "/mnt/crates"]
```

`b` opens the file browser.

## Waveforms

Three colour modes, cycle with `w`:

- **3-Band** — blue lows, amber mids, white highs layered as on a CDJ-3000.
- **RGB** — one blended colour per column: red lows, green mids, blue highs.
- **Blue** — a single blue waveform that brightens toward white as the highs rise.

## Track data

Analysis, the waveform and your cues are saved beside the audio as `<file name>.dj-tui.json`,
so they load instantly the second time and travel with the music if you move your library.
Editing or replacing the audio file invalidates them, and the track is analysed again.

## realtime audio

dj-tui asks for realtime on any thread it can.
To grant it on Fedora and most PipeWire desktops:

```sh
sudo usermod -aG pipewire $USER
```

Log out and back in, since group membership only applies to new sessions.

## Errors

dj-tui keeps a log at `~/.local/state/dj-tui/dj-tui.log`, or under `$XDG_STATE_HOME` if you
set one.

## Configuration

Copy and edit `config.toml.example` to `~/.config/dj-tui/config.toml`.

## MIDI controllers

Mappings are TOML files in `~/.config/dj-tui/mappings/`.
Copy `mappings/generic.toml` from this repository as a starting point, then find out what your
hardware sends:

```sh
cargo run -- --midi-learn
```

## Building and testing

```sh
cargo test --workspace
```
