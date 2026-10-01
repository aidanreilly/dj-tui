# dj-tui

A two-deck DJ console for the Linux terminal.

Two decks with CDJ-style cueing, hot cues, loops and key lock, a three-band isolator mixer with a master filter, colour waveforms drawn as real pixels where the terminal allows it, and tempo and key detection that runs as a track loads.

Supports WAV, FLAC, MP3, AAC/M4A, OGG Vorbis and AIFF files.

<img width="1433" height="841" alt="image" src="https://github.com/user-attachments/assets/e222814b-a197-4ac1-a4b2-d9729ccd2063" />


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

The keyboard has two layers. Mix mode is where you start:

![The dj-tui mix mode key map on a UK keyboard](images/keyboard.svg)

Browser mode, on `b` or `/`, hands the whole keyboard to the track list: letters type into the
filter, so every command moves onto `Alt`. `Esc` gives it back in one press.

![The dj-tui browser mode key map on a UK keyboard](images/keyboard-browser.svg)

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
| `[` / `]` | Halve and double the loop, keeping the in point. Shift (`{` / `}`) halves and doubles the fade length instead, shown as `FADE 8b` on the crossfader row: every automated fade runs that many beats. |
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
| `Backspace` | In the browser, edit the filter. Typing `bpm:124`, `bpm:124-128`, `key:9a` or `genre:house` narrows by that field; anything else is a fuzzy search over artist and title. |
| `Alt+s` / `Alt+S` | In the browser, change the sort column, and with Shift the direction. |
| `Alt+a` | In the browser, work through the list: read tags, look up what the tags left blank, then analyse. Press it again to stop. |
| `Alt+b` | In the browser, a tempo window around the playing deck: off, within 3 %, within 6 %. Half and double time count. |
| `Alt+k` | In the browser, show only the keys that would mix with the playing deck's. |
| `Alt+g` | In the browser, cycle the genres your library holds, and back to off. |
| `Alt+d` | In the browser, ask Discogs about the selected track's genre. |
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

In browser `Alt+s` and `Alt+S` sort, `Alt+a` reads and analyses, `Alt+f` swaps between full screen and the panel, `Ctrl+u` clears the filter.

## The browser

Point dj-tui at your music:

```toml
[library]
folders = ["~/Music", "/mnt/crates"]
```

`b` opens the file browser full screen, `/` opens it beside the decks.

It narrows two ways. `Alt+b` cycles a tempo window around whatever deck is playing, counting
half and double time, so a 64 BPM track still shows against 128. `Alt+k` keeps only the keys
that would mix with it. `Alt+g` cycles the genres your library holds. Whatever is on shows as
a chip in the panel title.

For anything more exact, type it: `bpm:124`, `bpm:124-128`, `key:9a` and `genre:house` filter
by that field, and anything else is a fuzzy search over artist and title. They combine, so
`bpm:124-128 bicep` is both.

`Alt+a` works through the list: it reads tags first, then looks up what the tags left blank,
then analyses. Press it again to stop.

### Genre

Genre comes from the file's own tags. For the files whose tags carry none, dj-tui can ask
Discogs instead, matched on artist and title. It is off until you turn it on, and nothing is
sent unless you press `Alt+a` or `Alt+d`:

```toml
[library]
discogs = true
```

```sh
export DJ_TUI_DISCOGS_TOKEN=...   # discogs.com, Settings then Developers
```

The token is read from the environment rather than the config file, so a config you copy
around carries no credential. Turning this on sends the artist and title of those files to
discogs.com. Answers are kept in `$XDG_STATE_HOME/dj-tui/discogs.json`, so nothing is asked
twice.

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
