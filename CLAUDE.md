# Working on dj-tui

Spec and milestone plan: `docs/spec.md`. M0 through M3 are done, and M4 analyses tempo and key
on load in `crates/analysis`. M5 brought loops, beat jump and quantize, with cues kept in the
track sidecar rather than the SQLite store the spec describes. M7 has sync, nudge and key lock,
and M9 has mappings, soft takeover, LED feedback and hotplug through JACK MIDI.

M8 has the browser with search,
sorting and batch analysis, and M10 has the raw ALSA backend and the device screen. Out of
scope, so not worth proposing again: jog scratching, two-card output, SysEx handshakes in mappings, the
hour-long stress run, and the effect slot, which was removed.

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

## Colours

`tui::theme` is the only place a colour is written down. The palette is Solarized, and the
constants under the sixteen values name roles rather than hues, so a widget asks for
`FOCUS_BORDER` and the mapping stays in one file. Nothing sets a background: the app draws on
whatever the terminal has, which is what keeps a light terminal usable. `crates/tui/tests/
solarized.rs` pins the values and the roles, so a colour written by hand somewhere else fails
there rather than only looking slightly wrong on screen.

The waveform palette reads the CDJ-3000's scheme through Solarized: blue lows, yellow mids,
pale highs. Blue mode's bright end is the warm light base rather than a blue-white, so only
its dark end carries the hue.

## Waveform rendering

`tui::pixel` rasterises the overview into RGBA images and `tui::Graphics` sends them through
the kitty graphics protocol when `detect_graphics()` finds it (Ghostty, kitty, WezTerm). Each
deck keeps a fixed kitty image id and only resends when the playhead reaches a new pixel
column. The glyph renderer in `tui::waveform` stays as the fallback and always draws first.
`[ui] graphics = "off"` forces glyphs.

The loader produces a per-band envelope alongside the peak envelope, and both renderers share
the colour rules that turn those bands into 3-Band, RGB or Blue. Downsampling averages bands
the same way it averages peaks, and a cell takes its colour from the row centres it covers.
`w` cycles the modes, and `[ui] waveform_mode` sets the one the app starts in.

## Loops

Loop lengths are counted in beats, so `ControlState` carries each deck's beat grid in frames
and the UI sends absolute frame positions. The engine deck wraps only on the step that crosses
the loop end, which is what lets a hot cue or a seek out of a loop keep playing. Quantize is a
UI-side snap of loop in points and jumps to the nearest beat, not something the engine knows
about. Both renderers draw the loop green: a tint plus edge lines in `tui::pixel`, a lit
background and brackets in `tui::deck`. `i` marks a loop in point and `I` closes the loop at
the playhead, which also sets the length the halve and double keys work from.

## Key lock

`engine::stretch` is a WSOLA stretcher: overlapping grains cut from the track, each slid a
little to line its waveform up with what has already been written, then overlap-added with a
Hann window that sums to one at half-grain overlap. The deck still advances its playhead at
the fader's rate, so position, loops and cues behave the same either way, and only the reading
changes. It runs only with key lock on and a rate away from one, which is why `k` at normal
speed is bit-identical to no key lock at all. `cargo run --release --example stretch_cost`
prints what it costs: around 2.5 % of real time per deck.

## Controllers

`crates/midi` is the mapping layer and touches no hardware: TOML in, `Action`s and absolute
`Control` moves out, which is why it is tested from strings. `src/controller.rs` joins it to
the app, and `crates/backend` owns the JACK MIDI ports at both ends. MIDI goes through JACK
rather than ALSA sequencer directly, which adds no dependency and picks up ALSA devices
through PipeWire anyway; `Running::connect_midi` rescans every two seconds, which is hotplug.

Knobs wait for soft takeover, so `controller.sync(&app)` has to run each frame to tell the
mapping where the UI holds each control. Lights are sent only on change. Mappings live in
`mappings/` beside the config file, and `mappings/generic.toml` in the repo is the starting
point to copy.

## Realtime scheduling

`crates/backend/src/realtime.rs` is the whole of it: `scheduling()` reads the calling thread's
policy, `request_realtime()` asks for SCHED_FIFO at `AUDIO_PRIORITY` (10, modest on purpose),
and `Scheduling` packs into a `u32` so an audio thread can report through an atomic with zero
meaning "has not run yet". Both are two syscalls and no allocation, so calling them once from
inside a callback is safe.

Who asks, and who can only look:

- ALSA owns its playback thread, so it asks at the top of the spawn closure before `play`.
- JACK does not own the callback thread, but that thread lives in our process, so the first
  callback asks when the server left it ordinary. It never overrides a server that already
  granted realtime, because the server's priority is deliberate and ours would be lower.
  pipewire-jack does not always elevate a client's callback thread even where PipeWire's own
  data loops are `RR 20`, which is exactly the case this covers.

A refusal is the ordinary case: `RLIMIT_RTPRIO` is granted to a group, not to everyone, so
`request_realtime` returns `EPERM` on a stock desktop and nothing stops. What it got goes on
the status line beside the rate and the xrun count, and a non-realtime thread logs one line
saying why it will glitch. That report is the point: the priority is usually not dj-tui's to
take, so saying what happened is more use than trying harder.

Deliberately not done: asking rtkit over D-Bus, which would need a D-Bus dependency to reach
the same cap the server already has; and `mlockall`, which is the other half of realtime
hygiene and belongs with it if it is ever added.

## Log file

`src/log.rs` appends timestamped lines to `$XDG_STATE_HOME/dj-tui/dj-tui.log`, rotating past a
megabyte. It is for what a session leaves behind, not for tracing: startup, failures and
warnings, and the xrun count at the end. `App::take_log` hands over what happened since the
last frame, so anything worth keeping is pushed there rather than only shown in the message
line. Nothing logs from the audio thread.

## Session file

`src/session.rs` writes `$XDG_STATE_HOME/dj-tui/session.toml`, which holds one path per deck
and nothing else. `main.rs` reads it after the audio starts, unless files were named on the
command line or `--demo` was, and loads each path through `App::load_path`, which is the same
route a browser load takes, so the sidecar comes with it.

Writing happens in the frame loop, whenever `App::deck_paths` differs from what was last
written, rather than on the way out: a session that ends in a crash is the one that most wants
its decks back. `deck_paths` reads the same `Persisted` records the cue saving uses, so it
names what the engine is playing rather than the last thing asked for, and a load that failed
leaves nothing behind. The decks are named a field at a time because TOML cannot write an
empty slot inside an array. Nothing here fails loudly: a missing, unreadable or nonsense file
means empty decks, which is where dj-tui started before the file existed.

## ALSA backend

`backend::alsa_backend` drives a card directly for running without a sound server, from a
thread of its own. Memory mapping is asked for and then actually tested at open time, because
the plugin devices advertise it in their hardware parameters and refuse the mapping itself;
when that happens the device is opened again for the copying transfer. The device is prepared
and its ring prefilled with silence before anything starts, which is what keeps the xrun
count at zero from the first period.

`Ctrl+D` opens the device screen. Changing device means reopening it, so the choice is
written into the config file with `config::with_device`, which edits only that line and
leaves the person's comments and ordering alone.

`cargo test -p backend --test alsa` skips the playback test unless `DJ_TUI_ALSA_TESTS=1`.

## Input layer

`crates/input` decides what every key means, including which mode the keyboard is in:
`Mode::Mix`, `Mode::Browser` and `Mode::Devices` in `keymap/mod.rs`, with a table per mode in
`keymap/mix.rs` and `keymap/browser.rs`. `App::on_key` is a dispatcher and holds no modal
state of its own. Browser mode owns the whole keyboard so letters type a search, which is what
frees the arrow keys for the faders, and its own commands sit on `Alt` plus a letter. `Esc`
leaves in one press and the filter stays on, drawn under the list.

Backtick, which sent the next key to the other deck, is removed. `Tab` is the only way to
reach the other deck. It saved one press per transition and was the keymap's only hidden
sticky state; the band swap took away its best case. Not worth proposing again.

`-`/`+` ride both decks' tempo by the same proportion, so a beatmatched pair stays matched
exactly: `Dir::Down` is the reciprocal of `Dir::Up` rather than `1 - step`, which is what makes
riding up and back return to where it started instead of creeping down each round trip, and
there is no rounding because the ratio between the decks has to survive. A move that would take
either deck out of its tempo range is refused whole, since clamping one and not the other is
how a matched pair comes apart. `,`/`.` are the focused deck's own pitch, `<`/`>` nudge, and
beat jump is on `j`/`J`.

Sync's phase alignment needs the other deck to be playing. A deck whose track has ended keeps
its grid with its position pinned at the end, so it is still a grid but no longer a clock;
lining up against that frozen phase used to seek the running deck back to the top of its track.
The phase meter takes the same view and goes quiet unless both decks are running.

Horizontal arrows are the crossfader, vertical arrows the focused deck's channel fader, with
`Shift` going hard to an end. `x`/`X` and `v`/`V` return the crossfader and the master filter to
neutral. The channel fader steps in dB, because a linear step is 0.4 dB at the top of the
travel and 26 dB at the bottom.

Stepped EQ gain is reachable only from MIDI. The kill switches name their deck rather than
following focus: `t`/`y`/`u` drop the lows, mids and highs on deck A and `T`/`Y`/`U` on deck B,
and a press toggles. `Action::Eq` and `Control::Eq` are untouched, so encoders and knobs behave
as before. `apply_many` exists beside `apply` for the actions that move more than
one control.

`Control` lives in `crates/input`, not `crates/midi`, because both paths name continuous
controls; `midi` re-exports it so mapping files are unaffected.

The deck and channel without focus are greyed out. The panel is dimmed whole by `grey_out` in
`tui::deck`, and the mixer columns through `column_styles` in `tui::screen`. Kitty waveforms
keep constant brightness when focus changes because changing an image retransmits the full
waveform; the active deck keeps grey title labels and a red border. `PixelWaveform` still
supports focus dimming,
but `Graphics` renders both waveforms at full brightness.

## Fades

`src/automation.rs` runs fades on the UI thread, ticked from `App::tick` with the frame delta
and from `App::tick_fades` in tests, so it needs no clock. Values go out through
`apply::set_control`, which keeps `ControlState` the only owner of absolute values and leaves
MIDI soft takeover working. The engine is not involved.

`Curve::Position` ramps the control's own travel, for the crossfader (whose configured curve
is already downstream) and for the master filter. `Curve::Decibel` ramps in dB with a −60 dB
floor, for channel faders, since `SetChannelFader` takes raw amplitude.

Lengths come from the deck's beat grid at its current rate and are fixed when the fade starts.
`MIN_FADE_SECS` is 0.7 s because `Mixer::process` applies fader gains as plain scalars with no
per-sample ramp, so a fade is a series of steps and that floor keeps each 30 fps step no larger
than one manual key press. Per-sample ramps in the engine would remove the floor and were
deferred, not rejected.

A fade dies on `Esc`, on any manual move of the same control from key or knob
(`manual_control`, and `set_control` cancels first), on a new track loading onto that deck, and
when another fade claims the control. At most one fade per control. The master filter's fade
is the one exception to the track-load clause: it belongs to no deck, so `cancel_deck` leaves
it running.

## Browser

`crates/library` is the data layer and has no database behind it: `scan` walks the configured
folders and every column comes from the sidecar beside each file, so a track that has never
been played still lists. `src/browser.rs` holds what changes as keys are pressed, and
`tui::BrowserPanel` draws it.

Browser mode is the keymap's, not the app's: see the input layer section above. Playlists are
m3u. Key highlighting compares each row against the key
of whichever deck is playing, deck A first.

`Alt+a` analyses everything with no tempo yet, one file at a time through the loader thread so
the machine stays usable, with progress in the panel title. Pressing it again stops the run.
A track counts as analysed once its sidecar holds a grid, so a file the detector could not
read last time is picked up again: `loader::analyse_file` is the batch entry point and it
ignores a saved analysis with no grid, which is how a better detector reaches a library that
has already been scanned. A deck load still takes the sidecar as it stands. The cost is that
a track with no beat in it is read again on every run.

## Master limiter

`dsp::Limiter` is the last thing on the master bus, and the cue bus blends in what it puts
out rather than the raw sum. The gain can never rise above what the current sample allows, so
the output cannot pass `LIMIT_CEILING` at all; only the recovery is smoothed. Channel meters
read pre-limiter, the master meter reads after it. A test that sets a level near full scale
should measure the cue bus, which is pre-fader and ahead of the limiter.

The filter is one control on the master bus rather than one per channel, sitting immediately
ahead of the limiter so the limiter still has the last word on level. The headphone cue is
tapped pre-fader, before the channels are summed onto the master, so it hears the filter only
through the cue mix, which is what a master filter does on hardware.

## Track sidecar

Analysis, the overview waveform, cues and the running loop live in `<file name>.dj-tui.json`
beside the audio,
written by `loader::sidecar`, so a library keeps its data when it moves between machines. An
`AudioId` of file size plus an FNV-1a hash of the first megabyte tells the loader when the
audio changed and the stored data no longer applies. Bump `SIDECAR_VERSION` when the meaning
of a stored field changes. Cue edits are saved from a background thread, never from the audio
thread.

## Detector accuracy

A candidate tempo is scored with the comb of its own line, its eighths and its sixteenths,
each weaker than the last. A beat has all three under it and a cross rhythm has only its own,
which is what stops a five-against-four acid pattern reading as 150 BPM over a 120 BPM track.
The search runs inside the configured range, so half and double time fold there rather than
through a list of multipliers.

Tempo is read in thirty-second windows and the median wins. A whole-file sum only holds for a
tempo that never moves, and a tape transfer that loses a beat and a half across a side smears
it away entirely. Windows that sit more than three percent from the median mean the track has
no one tempo, and it gets no grid at all.

`analysis::eval` scores tempo and key against reference annotations, and
`cargo run --release --example giantsteps -- <dataset dir>` runs it over a dataset, reading
annotations beside each file or under `annotations/tempo` and `annotations/key`. It exits
non-zero below the spec's targets of 90 % tempo within ±2 % and 65 % exact key. CI runs it
only when the `GIANTSTEPS_DIR` repository variable points at a copy of the datasets.

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

MSRV is 1.90, set by `quantette`, which arrives as `ratatui-image` 11.1 -> `icy_sixel` ->
`quantette` and is not optional there. dj-tui draws through the kitty protocol and never uses
sixel, so this is a floor paid for a code path that never runs; `icy_sixel` becoming optional
upstream is the way out. Do not lower it to 1.88 without checking that chain: the CI `msrv`
job pins the toolchain and `cargo test --workspace --locked` fails at resolve time, before any
compilation, which is why the failure reads as three unrelated crates rather than as one.

The workspace uses `resolver = "3"`, so `cargo update` prefers crates that build on the MSRV,
but it will not downgrade something already in the lock. When the `msrv` job goes red, compare
the ceiling against the floor:

```sh
cargo metadata --format-version 1 --locked | python3 -c "import json,sys;\
print(sorted(((p['rust_version'],p['name']) for p in json.load(sys.stdin)['packages'] \
if p.get('rust_version')), reverse=True)[:5])"
```

