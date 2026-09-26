# dj-tui: a two-deck DJ console for the Linux terminal

## 1. Scope

dj-tui is a DJ application for Linux terminals, driven primarily from the keyboard with MIDI controllers supported alongside. It plays two tracks at once, mixes them through a DJ-style mixer and sends the master and headphone signals to separate outputs on a soundcard. Tracks are analysed for tempo, beat grid and musical key when loaded, and the results are cached so the second load is instant.

dj-tui is licensed GPL-3.0-or-later, so GPL libraries such as Rubber Band and libKeyFinder can be linked freely.

Out of scope for v1: recording, streaming, video, timecode vinyl, HID-only controllers, more than two decks, macOS or Windows.

### Target environment

| Item | Target |
|---|---|
| OS | Linux, kernel 5.x+ |
| Audio servers | PipeWire (via its JACK layer), JACK2, raw ALSA |
| Terminals | Any truecolor terminal; kitty, foot, WezTerm, Alacritty tested first |
| Minimum terminal size | 100 × 40 cells (120 × 44 recommended) |
| Formats | MP3, FLAC, WAV, AIFF, OGG Vorbis, Opus, AAC/M4A |
| Latency goal | 128–256 frames at 48 kHz (2.7–5.3 ms per buffer) |

## 2. Screen layout

```
┏━ ▶ DECK A ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 124.00 ▲+0.0%  8A ┓┌ MIXER ─────────────────┐
┃ Artist – Title                                   03:12  -02:41 ┃│      A           B     │
┃ ⣿⣷⣶⣤⣀⣀⣤⣶⣿⣿⣷⣶⣤⣀⣀⣤⣶⣿⣿⣷⣶⣤⣀⣀⣤⣶⣿⣿⣷│⣶⣤⣀⣀⣤⣶⣿⣿⣷⣶⣤⣀⣀⣤⣶⣿⣿⣷⣶⣤⣀⣀⣤⣶⣿⣿ ┃│ TRIM ▮▮▮▮▯   ▮▮▮▯▯     │
┃ ⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶│⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿ ┃│ HI   ▮▮▮▯▯   ▮▮▮▯▯     │
┃ ⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶│⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿ ┃│ MID  ▮▮▮▯▯   ▮▮▮▮▯     │
┃ ⠛⠿⣿⣿⠿⠛⠉⠉⠛⠿⣿⣿⠿⠛⠉⠉⠛⠿⣿⣿⠿⠛⠉⠉⠛⠿⣿⣿⠿│⠛⠉⠉⠛⠿⣿⣿⠿⠛⠉⠉⠛⠿⣿⣿⠿⠛⠉⠉⠛⠿⣿⣿⠿⠛⠉⠉ ┃│ LOW  ▮▯▯▯▯   ▮▮▮▯▯     │
┃   ▲1      ▲2              ▲4                            ▲8     ┃│ FLT  ─┼─     ──┼       │
┃ [1][2][ ][4][ ][ ][ ][8]   LOOP 4 ●   SYNC   Q   FX ECHO 1/2 ON ┃│                        │
┗━ PHASE ━━━━━━━━━━━━━━ ░░░░░░▓▓│░░░░░░░ ━━━━━━━━━━━━━━━━━━━━━━━━━┛│                        │
┌ DECK B ──────────────────────────────────── 126.00 ▲-1.6%  9A ─┐│ VOL  █   ▌▌ ▌▌   █     │
│ Artist – Title                                   01:05  -04:50 ││      █   ▌▌ ▌▌   █     │
│ ⣀⣤⣶⣿⣿⣷⣶⣤⣀⣀⣤⣶⣿│⣿⣷⣶⣤⣀⣀⣤⣶⣿⣿⣷⣶⣤⣀⣀⣤⣶⣿⣿⣷⣶⣤⣀⣀⣤⣶⣿⣿⣷⣶⣤⣀⣀⣤⣶⣿⣿⣷⣶ ││      ▯   ▌▌ ▌▌   █     │
│ ⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿│⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶ ││ CUE  ●           ○     │
│ ⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿│⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶⣿⣿⣿⣿⣿⣿⣶⣶ ││ XFADE A ━━━━╋━━━ B     │
│ ⠛⠿⣿⣿⠿⠛⠉⠉⠛⠿⣿⣿⠿│⠛⠉⠉⠛⠿⣿⣿⠿⠛⠉⠉⠛⠿⣿⣿⠿⠛⠉⠉⠛⠿⣿⣿⠿⠛⠉⠉⠛⠿⣿⣿⠿⠛⠉⠉⠛⠿ ││                        │
│   ▲1  ▲3                                                        ││ PHONES cue ━━╋━━ mst   │
│ [1][ ][3][ ][ ][ ][ ][ ]   LOOP –     SYNC ● Q   FX REVERB   OFF ││                        │
└────────────────────────────────────────────────────────────────┘└────────────────────────┘
┌ BROWSER  ~/Music/house ──────────────────────────────────────────────────────── /search ┐
│ > Artist          Title                              BPM     Key   Len                 │
│   ...                                                                                  │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

Deck A sits above deck B, each spanning the full width left of the mixer. The focused deck gets a heavy border and a ▶ marker. Every deck shows a single whole-track waveform four rows tall, mirrored around its centre line, with hot cue markers underneath. A one-row phase meter between the decks shows how far deck B's beat sits ahead of or behind deck A's, which stands in for the visual alignment a scrolling view would give. The mixer runs as a vertical strip on the right with channel faders side by side. The browser takes whatever vertical space remains and can be toggled full screen with `b`.

Below 110 columns the mixer strip collapses into a two-row horizontal bar between the decks and the browser.

## 3. Functional requirements

### 3.1 Decks

Each deck supports play/pause, the CDJ-style main cue, tempo fader (range selectable ±8, ±16, ±50 %), pitch bend nudge, key lock, sync, quantize, eight hot cues, loops and beat jump.

Main cue behaviour copies Pioneer players. While paused, pressing cue stores the current position. While playing, pressing cue returns to the stored point and pauses. Holding cue while paused previews from that point and snaps back on release.

### 3.2 Waveforms

One static view per deck showing the entire track, drawn in braille characters. Each cell holds a 2 × 4 dot grid, so a 90-column panel gives 180 horizontal samples and four mirrored rows give 8 dot steps either side of the centre line. Because the waveform only changes on load, each frame redraws just the playhead column and the dimming boundary. Clicking or pressing `g` plus a digit seeks to that tenth of the track.

#### Colour modes

The three modes follow the ones on Pioneer's CDJ-3000 and rekordbox, selectable with `W` or in the config.

| Mode | Look | How it's computed |
|---|---|---|
| 3-Band (default) | Lows in dark blue, mids in amber, highs in white, drawn as layers | Each band's envelope is drawn at its own height; highs sit on top of mids, mids on top of lows |
| RGB | One blended colour per column | Low energy drives red, mid drives green, high drives blue, normalised per track |
| Blue | Single-colour blue waveform | Height from full-band RMS, with columns tinted toward white as high-frequency content rises |

Braille allows one foreground colour per cell, so 3-Band layering happens at cell granularity. For each cell the renderer picks the topmost band whose envelope reaches into that cell's vertical range. The outer cells of a kick come out blue while hi-hats show as a white core near the centre, which reads close to the CDJ picture at terminal resolution.

#### Overlays

These match what a CDJ shows on its overview:

- Played portion drawn at about 40 % brightness, unplayed portion at full.
- Playhead as a bright vertical bar through all four rows.
- Hot cues as coloured markers in the row beneath, using each cue's assigned colour (eight defaults matching the CDJ palette, editable).
- Main cue as an orange marker, and memory cues as red markers.
- Active loop shaded orange across its span.
- A beat and bar counter in the status row, counting down to the next memory cue.
- End-of-track warning: the unplayed part of the overview flashes red during the last 30 seconds (configurable), exactly as a CDJ does.

Phrase colouring (intro, verse, chorus, outro bands) needs structure analysis and is listed under post-v1.

### 3.3 Mixer

Per channel you get trim, a three-band isolator EQ with full kill, a one-knob filter (left of centre is low-pass, right is high-pass, dead zone in the middle), channel fader, headphone cue button and a peak meter. The master section adds a crossfader with three curve shapes, master volume, a brickwall limiter, and a headphone section with cue/master blend and its own level.

### 3.4 Effects

One effect slot per deck, post-EQ and pre-fader. v1 ships four units, all tempo aware where it makes sense:

- **Echo**: tape-style feedback delay, time set in beats (1/8 up to 2), feedback and wet controls.
- **Reverb**: Freeverb-style algorithm with size and damping.
- **Flanger**: LFO period locked to beats.
- **Bitcrusher**: sample rate reduction plus bit depth, useful for build-ups.

Every effect has dry/wet and an on/off toggle. Echo and reverb keep ringing out after being switched off (tails), since cutting them dead sounds wrong in a mix.

### 3.5 Analysis

Runs in a background worker pool on track load and optionally in batch over the whole library.

**Tempo and beat grid.** Compute an onset strength envelope from spectral flux on a log-frequency spectrogram (hop around 5.8 ms). Autocorrelate the envelope and weight candidates with a prior favouring 70–180 BPM, then fold the result into a user-configurable range such as 85–175 to settle half/double-time ambiguity. Find grid phase by cross-correlating an impulse train at the chosen period against the envelope. Assume a constant tempo for v1, which suits most electronic music, and let the user shift the grid or halve/double the BPM by hand.

**Key.** Build a 12-bin chromagram from a constant-Q transform over roughly 50 Hz to 5 kHz, sum it over the track with a bias away from the intro and outro, and correlate against the 24 major and minor key profiles. Temperley or EDM-tuned profiles beat the classic Krumhansl ones on dance music. Since the project is GPL, libKeyFinder can sit behind the same trait as a second detector, and whichever scores better on GiantSteps becomes the default. Show results in Camelot notation (8A, 9B) by default, with standard notation as an option.

**Waveform data.** Store peak and RMS per band at about 20 points per second. On load or terminal resize, reduce that to one value per braille dot column for the current panel width.

Accuracy targets are 90 % tempo accuracy (±2 %, allowing octave errors to be flagged) and 65 % exact key accuracy, measured against the GiantSteps tempo and key datasets.

### 3.6 Soundcard support

dj-tui needs two stereo outputs: master and headphones. Three routing modes cover real hardware.

1. **Multichannel card** (e.g. a 4-out DJ interface): master on outputs 1/2 and headphones on 3/4. This is the normal setup.
2. **Two cards**: master on one device and headphones on another, such as laptop jack plus USB dongle. Clock drift between them is handled by resampling the headphone bus slightly, which is only possible cleanly through JACK/PipeWire or with an adaptive resampler on the ALSA backend.
3. **Split mono on one stereo card**: master summed to mono on the left channel and cue summed to mono on the right. A Y-splitter cable finishes the job.

Backends, in order of preference:

- **JACK API**, which transparently covers PipeWire through `pipewire-jack`. Ports register as `dj-tui:master_L`, `dj-tui:master_R`, `dj-tui:cue_L`, `dj-tui:cue_R`, and auto-connection to physical ports happens per the config.
- **Raw ALSA** via `hw:` devices for people running neither server. Uses mmap mode with a configurable period size and period count.

A setup screen lists devices, channel counts and supported rates, lets you assign outputs and shows the xrun counter live.

### 3.7 Library and persistence

The browser is a directory tree plus a track list with sortable columns (artist, title, BPM, key, length) and incremental search. Tracks in a compatible key to the playing deck (same Camelot number, ±1, or relative major/minor) get highlighted.

Everything durable lives in SQLite at `~/.local/share/dj-tui/library.db`: track metadata, analysis results, waveform blobs, hot cues, loops and grid edits. Rows are keyed by a content hash (xxHash3 over the decoded first 30 seconds plus file length) so renames and moves keep their cues. Config goes in `~/.config/dj-tui/config.toml`.

### 3.8 Controls

Controls work on a focused deck. `Tab` moves focus between A and B, and every deck and channel key applies to whichever deck has it. One set of keys covers both decks, which halves what there is to learn and frees the keyboard for mixer controls that used to need two keys each.

The catch is anything done to both decks close together, such as a bass swap or starting B on A's downbeat. For that, backtick (`` ` ``) sends the next key to the unfocused deck without moving focus, so dropping the bass on B while A stays focused is `` ` `` then `u`.

Continuous controls follow one rule: lowercase turns it down, Shift turns it up. Alt gives a fine step on tempo.

**Global**

| Action | Key |
|---|---|
| Switch focused deck | `Tab` |
| Apply next key to other deck | `` ` `` |
| Crossfader | `←` / `→` (Shift snaps to the end) |
| Browser move / load into focused deck | `↑` / `↓`, `Enter` |
| Search / browser full screen | `/`, `b` |
| Waveform colour mode | `W` |
| Help / quit | `?`, `Ctrl+Q` |

**Focused deck**

| Action | Key |
|---|---|
| Play/pause | `Space` |
| Cue (hold to preview) | `c` |
| Hot cue 1–8: trigger, or set if empty | `1`–`8` |
| Clear hot cue | `Alt+1`–`8` |
| Tempo down / up | `-` / `+` |
| Nudge back / forward | `,` / `.` |
| Sync / quantize / key lock | `s`, `q`, `k` |
| Loop on/off (4 beats) / halve / double | `l`, `[`, `]` |
| Beat jump back / forward | `<` / `>` |
| FX on/off / next effect / wet | `f`, `F`, `9` / `0` |
| Seek to tenth of track | `g` then digit |

**Focused channel**

| Action | Key |
|---|---|
| Trim | `r` / `R` |
| EQ high / mid / low | `t` / `T`, `y` / `Y`, `u` / `U` |
| EQ kill high / mid / low | `Alt+t`, `Alt+y`, `Alt+u` |
| Filter toward low-pass / high-pass | `o` / `O` |
| Channel fader | `v` / `V` |
| Headphone cue | `m` |

Hold-to-preview and press-and-hold behaviours need key release events. Crossterm can request them through the kitty keyboard protocol, which kitty, foot, WezTerm and recent Alacritty support. On other terminals those controls fall back to toggle behaviour and the status bar says so.

The keyboard is the primary interface and everything in the app is reachable from it. MIDI (section 3.9) maps onto the same set of actions.

### 3.9 MIDI controllers

Both input paths produce the same `Action` values (`Play(Deck)`, `SetEq(Deck, Band, f32)`, `HotCue(Deck, n)` and so on), so any action the keyboard can trigger can also be mapped to a controller. Keyboard focus doesn't apply to hardware. A controller has physical controls for each deck, so mappings name decks explicitly, and a mapping may also target `focused` for single-deck controllers or spare buttons.

**Transport.** MIDI comes in through the ALSA sequencer, which also works under PipeWire. On the JACK backend, JACK MIDI ports are offered as well, giving sample-accurate timestamps. Devices are hot-plugged: the sequencer's announce port tells dj-tui when a controller appears or disappears, and a saved mapping is reattached automatically by port name.

**Control types the mapping layer understands:**

| Type | Typical source | Handling |
|---|---|---|
| Button | Note on/off | Press and release both delivered, so hold-cue and momentary FX work on any terminal |
| Absolute | 7-bit CC | Knobs and faders, scaled to the parameter range |
| High resolution | 14-bit CC pair (MSB/LSB) | Pitch faders and crossfaders on better controllers |
| Relative | Encoder CC | Two's complement, sign-magnitude and offset-64 variants, chosen per control |
| Jog wheel | Touch note plus relative CC | Touching the platter scratches; turning the edge nudges pitch |
| Shift layer | Any button | A mapping-defined shift button switches controls to a second action set |

**Soft takeover.** Keyboard and hardware can both move the same fader. When they disagree, a hardware fader has no effect until it passes through the current value, which stops volume jumps when you grab a fader after using the keyboard. The mixer strip shows a small marker where the hardware control currently sits.

**LED feedback.** Mappings can send output back to the controller: play and cue lights, hot cue pad colours (as velocity values, per the device's palette), sync, loop, FX state and VU meters where the hardware has them. Output is sent only on change and rate-limited to keep USB traffic low.

**Scratching.** Jog wheels drive the deck's read head directly. The engine smooths incoming wheel movement over a few milliseconds and supports forward and backward playback at any rate, with key lock bypassed while scratching. This is the one deck behaviour that exists only on MIDI.

**Mappings** are TOML files in `~/.config/dj-tui/midi/`, one per controller:

```toml
[device]
name = "DDJ-FLX4"
match = "DDJ-FLX4 MIDI 1"

[[control]]
midi = { type = "note", channel = 1, note = 0x0B }
action = "play"
deck = "A"
led = { type = "note", on = 127, off = 0 }

[[control]]
midi = { type = "cc14", channel = 1, msb = 0x00, lsb = 0x20 }
action = "tempo"
deck = "A"
invert = true
```

**MIDI learn.** `Ctrl+L` opens learn mode. Choose an action from a list or press its keyboard key, move the hardware control, and the binding is written to the active mapping file. This makes it quick to support a controller nobody has written a mapping for yet.

v1 ships a generic mapping for plain knob-and-pad controllers and one full mapping for a common class-compliant DJ controller. Controllers that already work with Mixxx on Linux are good candidates, since their MIDI behaviour is well documented.


## 4. Architecture

### 4.1 Stack

| Concern | Choice | Notes |
|---|---|---|
| Language | Rust | Memory safety plus control over allocation on the audio thread |
| TUI | ratatui + crossterm | Cell diffing keeps redraw cost low |
| Audio | `jack` crate, `alsa` crate | Behind a common `AudioBackend` trait |
| Decoding | symphonia | Pure Rust, covers every format listed |
| Resampling | rubato | Used at load time to reach the session rate |
| Time-stretch | Rubber Band 3.x via FFI | R3 engine for key lock, R2 as a lighter fallback when CPU is tight |
| Key detect reference | libKeyFinder via FFI | Benchmark for the in-house detector, and a selectable alternative if it scores higher |
| FFT | rustfft / realfft | Analysis only |
| Database | rusqlite | Bundled SQLite |
| RT messaging | rtrb ring buffers, atomics | No locks on the audio thread |
| MIDI | `alsa` crate (sequencer API), `jack` crate MIDI ports | Hotplug via sequencer announce events |
| RT-safe freeing | basedrop | Old track buffers get dropped on a collector thread |

### 4.2 Threads

```
 ┌──────────┐    commands     ┌────────────┐
 │ MIDI in  │ ──────────────▶ │            │
 └──────────┘     (rtrb)      │            │
 ┌──────────┐ commands (rtrb) │            │  frames  ┌──────────┐
 │ UI thread│ ──────────────▶ │ audio (RT) │ ───────▶ │ soundcard│
 │ ratatui  │ ◀────────────── │  callback  │          └──────────┘
 └────┬─────┘  state snapshot └─────▲──────┘
      │         (atomics)           │ Shared<TrackBuffer>
      ▼                             │
 ┌──────────┐  decoded buffers ┌────┴───────┐
 │ analysis │ ◀─────────────── │  loader    │ ◀── disk
 │ pool     │ ───▶ SQLite      │ (decode +  │
 └──────────┘                  │  resample) │
                               └────────────┘
```

The audio callback owns deck and mixer state outright. The UI sends commands (`Play(A)`, `SetEq(B, Low, 0.3)`, `LoadTrack(A, handle)`) and reads back a snapshot of playhead positions, meter peaks and loop state published through atomics once per callback. MIDI input runs on its own thread with a separate ring buffer into the engine, so a hardware cue press never waits for a UI frame. The UI learns about MIDI-driven changes from the same state snapshot it already reads, and LED output is generated from that snapshot too. Parameter changes are smoothed over about 10 ms inside the engine to avoid zipper noise.

Tracks are decoded completely into memory as interleaved f32 at the session rate. A ten-minute stereo track at 48 kHz costs about 230 MB, so two decks plus one preloaded track stay comfortably under a gigabyte. This makes scratching, reverse and instant hot cue jumps trivial since there's no disk streaming to manage.

### 4.3 Signal flow per deck

```
TrackBuffer → read head (rate, stretch) → trim → EQ isolator → filter → FX → fader ─┬→ crossfader → master bus → limiter → master out
                                                                                     └→ (pre-fader, if CUE) → cue bus → blend w/ master → cue out
```

The EQ is a Linkwitz-Riley 4th-order crossover at 250 Hz and 2.5 kHz, splitting into bands with independent gain. Summing them back unmodified is phase-flat, which a stack of peaking filters can't manage.

### 4.4 Crate layout

```
dj-tui/
  crates/
    engine/     # deck, mixer, fx, RT-safe; no I/O, fully testable offline
    backend/    # JACK and ALSA implementations of AudioBackend
    input/      # Action enum, keymap, MIDI mapping, learn mode, soft takeover
    analysis/   # bpm, grid, key, waveform peaks
    library/    # SQLite schema, hashing, scanning
    tui/        # widgets, layout, input mapping
  src/main.rs   # wiring, config, CLI
```

Keeping `engine` free of I/O means every DSP block can be driven by an offline test harness that renders to a WAV and compares against a reference.

## 5. Implementation plan

Estimates assume one experienced developer working part time.

| # | Milestone | Deliverable | Estimate |
|---|---|---|---|
| M0 | Skeleton | Workspace, GPL-3 licence headers, `cargo deny` licence policy, config loading, ratatui shell with empty panels, logging to file | 1 wk |
| M1 | One deck plays | symphonia decode, rubato resample, JACK backend, play/pause/seek, xrun counter | 2 wk |
| M2 | Two decks and mixer | Second deck, isolator EQ, filter, faders, crossfader, meters, cue bus, routing modes 1 and 3 | 2 wk |
| M3 | Waveforms | Band peak generation, braille overview, 3-Band/RGB/Blue colour modes, CDJ overlays and end-of-track warning, phase meter | 1.5 wk |
| M4 | Analysis | Onset envelope, tempo estimate, grid phase, chromagram key detect, GiantSteps evaluation script | 3 wk |
| M5 | Cues and loops | Main cue, hot cues, auto and manual loops, beat jump, quantize, SQLite persistence | 2 wk |
| M6 | Effects | Echo, reverb, flanger, bitcrusher with tails and beat sync | 2 wk |
| M7 | Sync and key lock | Tempo and phase sync, time-stretch integration, pitch bend | 2 wk |
| M8 | Browser | Directory tree, track table, search, sort, key compatibility highlight, batch analysis | 2 wk |
| M9 | MIDI | Action layer shared with keyboard, ALSA seq and JACK MIDI input, hotplug, mapping format, soft takeover, LED output, jog scratching, learn mode, shipped mappings | 2.5 wk |
| M10 | ALSA backend and polish | Raw ALSA with mmap, two-card mode with drift resampling, device setup screen, help overlay | 2 wk |

That's roughly 22 weeks to a usable v1. After M3 the tool is already good enough to mix by ear with two decks, which is a sensible point for dogfooding.

### Post-v1

Phrase analysis with CDJ-style phrase colouring on the overview, recording the master to FLAC, variable-tempo beat grids, a stem-free vocal/instrument filter, Unix socket for scripting.

## 6. Testing

- **Offline DSP tests**: render each engine block to a buffer and check against golden files with a tolerance, including EQ flatness and crossfader curve shapes.
- **RT safety**: wrap the callback with `assert_no_alloc` in debug builds so any allocation panics during testing.
- **Stress runs**: an hour of continuous two-deck playback with random loop and hot cue commands fired at the engine, at 64-frame buffers, logging xruns.
- **Analysis accuracy**: a CI job runs the detector over the GiantSteps datasets and fails if accuracy drops more than two points.
- **MIDI mappings**: recorded controller streams replayed through the mapping layer, checking the resulting actions and LED output.
- **TUI snapshots**: ratatui's `TestBackend` renders layouts at fixed sizes for snapshot comparison.

## 7. Risks

**Time-stretch quality and CPU.** Rubber Band's best engine is heavy. Run it only on decks with key lock on, and keep a plain resampling path when key lock is off.

**GPL compliance.** Distributing binaries means shipping the corresponding source and keeping every dependency GPL-compatible. A `cargo deny` licence check in CI catches incompatible crates before they land.

**Key release events.** Terminals without the kitty protocol can't report key-up, which hurts hold-cue and momentary effects. The toggle fallback covers it on the keyboard, and MIDI buttons always report release.

**Controllers that aren't plain MIDI.** Some controllers need a vendor SysEx handshake before they send anything, and a few speak HID only. Mappings can include an init SysEx block for the first case. HID-only hardware is out of scope for v1.

**Clock drift with two cards.** Direct ALSA across two devices needs adaptive resampling to stay aligned, which is fiddly. Recommend JACK/PipeWire for that mode and treat the ALSA version as best effort.
