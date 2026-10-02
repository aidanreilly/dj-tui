# Configuration

Copy `config.toml.example` to `~/.config/dj-tui/config.toml`, or under `$XDG_CONFIG_HOME` if you
set one, and edit it there. Every setting is commented in the example file. The sections cover:

| Section | Covers |
| --- | --- |
| `[audio]` | Backend (JACK or raw ALSA), card, sample rate, buffer size, and where the master and headphone cue outputs are connected at startup. |
| `[ui]` | Waveform colour mode, the pixel waveform toggle, and how early a deck warns that a track is ending. |
| `[deck]` | Tempo fader range: 8, 16 or 50 percent. |
| `[mixer]` | Crossfader curve and the fade length in beats. |
| `[midi]` | Whether controllers are looked for, which mappings load, and soft takeover. |
| `[library]` | Folders the browser lists, and the Discogs lookup. See [the browser](browser.md). |

`Ctrl+D` opens the audio device screen, and the card you pick there is written back into the
config file.

## MIDI controllers

Mappings are TOML files in `~/.config/dj-tui/mappings/`. Copy `mappings/generic.toml` from this
repository as a starting point, then find out what your hardware sends:

```sh
cargo run -- --midi-learn
```

Soft takeover is on by default, so a knob has to reach the value on screen before it takes
control. That stops a controller whose knobs sit somewhere else from jumping a fader on first
touch.

## Track data

Analysis, the waveform and your cues are saved beside the audio as `<file name>.dj-tui.json`.
They load instantly the second time, and they travel with the music if you move your library.
Editing or replacing the audio file invalidates them, and the track is analysed again.

## Logs

dj-tui keeps a log at `~/.local/state/dj-tui/dj-tui.log`, or under `$XDG_STATE_HOME` if you set
one. That is the first place to look when something goes wrong.
