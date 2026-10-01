# Changelog

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
