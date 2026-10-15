# Changelog

## 0.1.4

`n` turns the number row into nine automatic transitions from the playing deck into the other one: bass swap, long blend, filter blend, jump cut, drop swap, loop out, tempo bridge, brake, and Auto, which picks one to suit the pair. Each works out where to start from your hot cues, the track's sections and its phrase lines, brings the next track to tempo, and rides the EQ and faders across. A tempo gap, a key clash, a missing beat grid or a short runway bends the plan, and the list under the mixer shows what each key would do right now. One tap arms a transition for its downbeat, and moving any control it drives hands that control back to you. `[transitions] slots` in the config file changes what each key does and how long the blends run. See [transitions](docs/transitions.md).

Bars and sections now come from the low band of the overview waveform when a track loads, so intros, breakdowns, drops and outros are known without anything new in the sidecar.

The mixer folds trim and EQ into one row that reads `flat` until a band moves, then lights each band's letter for a cut, a boost or a kill. `[ui] eq_rows = "always"` brings the full rails back for a controller with EQ knobs.

With `output = "decks"` the transition keys refuse, since a hardware mixer does the mixing and a blend would play both tracks at full level.

## 0.1.3

`[audio] output = "decks"` sends deck A to outputs 1 and 2 and deck B to 3 and 4, untouched,
so a hardware DJ mixer does the crossfading, EQ and cueing. Each deck arrives as the player
makes it: pitch, key lock, loops and cues apply, and the whole channel strip and master bus
are bypassed, including the limiter. The mixer panel becomes two output meters, since none of
its controls reach the audio any more. It needs four outputs and defaults to off, so a config
without the key runs the path it ran before.

The documentation moved out of the README into `docs/`, which now covers installing, the full
key reference, the browser, waveforms and configuration. What stays in the README is a
quickstart of the keys needed to mix two tracks, and links into the rest.

Two things that read as broken audio and are not, now written down: the mixer collapses below
110 columns and takes the headphone mix with it, and `m` cues each deck separately, so cueing
both sends both to the headphones. The headphone mix also starts at an even blend of the cue
bus and the master rather than at the cue.

Four-output soundcards have a section of their own. The ALSA backend needs `plughw:` rather
than `hw:` on any card that offers no 32 bit float format, which is most USB interfaces, and
`routing = "auto"` under PipeWire usually finds the built-in card rather than the interface.

## 0.1.2

The README leads with the built binary now, and says how to verify it. Before this the only
documented way in was to compile the thing.

Browser mode has a keyboard diagram of its own. It is a second keyboard layer, and since
filtering landed it carries eight commands on `Alt` plus a typed field syntax. No picture
showed any of it.

## 0.1.1

The first release with a binary attached. A tag builds an x86_64 Linux tarball and its
checksum. The runner is an old one on purpose, so the glibc the binary links against is not a
floor most people fall through.

The job refuses to publish if JACK ever becomes a link-time dependency. JACK is loaded at
runtime, and that is what lets one tarball serve machines with a JACK server and machines
without one.

The automated fade length ran into the crossfader's B and read `BFADE 8`. It also gave no
sign that the number counts beats. It reads `FADE 8b` and keeps its distance.

## 0.1.0

First tag, and no binary with it: this one predates the release job.

Two decks with CDJ-style cueing, hot cues, loops, beat jump and key lock. A three-band
isolator mixer with kills, a master filter and a limiter. Tempo and key detection on load,
with beat grids. MIDI controller mappings with soft takeover and LED feedback. JACK and ALSA
output.

A library browser that filters by tempo, key and genre. Genre comes from a file's own tags,
or from a Discogs release matched on artist and title.
