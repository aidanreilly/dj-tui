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

## Soundcards with four outputs

dj-tui wants four output channels: master on 1 and 2, headphone cue on 3 and 4. A two-output
card can still work through `routing = "split"`, which puts a mono master on output 1 and a
mono cue on output 2 for a Y-splitter.

Either backend reaches a four-output card, and they fail in different ways.

### The ALSA backend

Driving the card directly needs no port names. The backend asks for four channels and maps
them itself, so `routing` is ignored.

```toml
[audio]
backend = "alsa"
device = "plughw:1,0"
sample_rate = 48000
buffer_frames = 256
```

Use `plughw:`, not `hw:`. dj-tui asks ALSA for 32 bit float and gives up if the card refuses,
and most USB interfaces offer only S32 or S24. The plug layer converts, while `hw:` fails with
`no 32 bit float format`. Find the card number in `/proc/asound/cards` or with `aplay -l`.

This takes the card exclusively, so nothing else on the desktop can play through it while
dj-tui runs.

### The JACK backend

Under a bare JACK server, `routing = "auto"` finds the right ports. Under PipeWire it often
does not: `auto` takes the first four physical playback ports JACK reports, and on a laptop
the built-in card usually owns them. Name the ports instead.

```toml
[audio]
backend = "jack"
routing = "explicit"
master_ports = ["<node>:playback_FL", "<node>:playback_FR"]
cue_ports    = ["<node>:playback_RL", "<node>:playback_RR"]
```

`pw-link -i` lists every port name, so copy them from there. They change if you switch the
card's PipeWire profile.

### Example: Focusrite Scarlett 4i4

The 4i4 has four analogue outputs. Outputs 1 and 2 are the Monitor jacks on the back, and
outputs 3 and 4 feed the headphone jack on the front, which is the layout dj-tui expects:
speakers on the master, headphones on the cue.

Out of the box the card mirrors the monitor mix into the headphones, so outputs 3 and 4 both
carry PCM 1 and 2. Leave it that way and the cue is silent while the master plays in the
headphones. Point the headphone pair at its own PCM channels:

```sh
amixer -c 1 cset numid=24 "'PCM 3'"   # Analogue Output 03, Headphones L
amixer -c 1 cset numid=25 "'PCM 4'"   # Analogue Output 04, Headphones R
```

Check the card number and the control numbers for your unit, since both vary:

```sh
amixer -c 1 controls | grep 'Analogue Output'
amixer -c 1 cget numid=24
```

`sudo alsactl store 1` keeps the routing across a reboot. Then the ALSA config above works as
written, with `plughw:1,0` for a card that reports as card 1.

## MIDI controllers

Mappings are TOML files in `~/.config/dj-tui/mappings/`. Copy `mappings/generic.toml` from this
repository as a starting point, then find out what your hardware sends:

```sh
dj-tui --midi-learn
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
