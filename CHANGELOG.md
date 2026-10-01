# Changelog

## 0.1.2

Documentation for the release itself: the README says a built binary exists and how to check
it, rather than only how to compile one, and browser mode has a keyboard diagram of its own.
Browser mode is a second keyboard layer, and since filtering landed it carries eight commands
on `Alt` and a typed field syntax that no picture showed.

## 0.1.1

The first release with a binary attached. A tag now builds an x86_64 Linux tarball and its
checksum, on a runner old enough that the glibc it links against is not a floor most people
fall through. The job refuses to publish if JACK ever becomes a link-time dependency rather
than one loaded at runtime, since that is what lets one tarball serve machines with and
without a JACK server.

Fixed: the automated fade length ran into the crossfader's B as `BFADE 8`, and gave no sign
that the number counts beats rather than seconds. It reads `FADE 8b` and keeps its distance.

## 0.1.0

First tag. No binary: it predates the release job.

Two decks with CDJ-style cueing, hot cues, loops, beat jump and key lock. A three-band
isolator mixer with kills, a master filter and a limiter. Tempo and key detection on load,
with beat grids. MIDI controller mappings with soft takeover and LED feedback. JACK and ALSA
output. A library browser that filters by tempo, key and genre, where genre comes from a
file's own tags or from a Discogs release matched on artist and title.
