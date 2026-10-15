# Transitions

`n` gives the number row to nine transitions. Each key works out a whole mix from the deck
that is playing into the other one: where it starts, where the next track comes in, how the
EQ and faders move, and when the old track stops. Press `n` again to get the hot cues back.
Everything else on the keyboard keeps working in the layer, so you can ride a fader or kill a
band while a transition runs.

The list under the mixer shows what each key would do right now, and it updates as the decks
move. `TRANSITIONS` leads the status line while the layer is on.

With `output = "decks"` the keys refuse and say why: a hardware mixer is doing the mixing, so the faders and EQ a transition moves never reach the sound, and a blend would play both tracks at full level.

## The keys

| Key | Kind | What it does |
| --- | --- | --- |
| `1` | Bass swap | 16 bars. The next track comes in with its lows held back, the lows swap on the downbeat halfway through, and the old track fades out over the rest. |
| `2` | Long blend | 32 bars. A slow three-band handover: mids eased in, highs on the old track pulled back, lows swapped halfway. |
| `3` | Filter blend | 16 bars. The next track arrives as highs alone and opens up, while the old one closes down to its lows. Done with the EQ, since the mixer has one filter on the master rather than one per channel. |
| `4` | Jump cut | A clean cut on the next four-bar line. |
| `5` | Drop swap | Waits for the old track's breakdown to end and cuts straight into the next track's drop. |
| `6` | Loop out | 8 bars. Loops four bars of the old track and blends over the loop. For when the track is running out. |
| `7` | Tempo bridge | 8 bars. For two tracks further apart than the tempo fader reaches. Rides the old track's tempo across over eight bars, with key lock on, then blends for the length of the slot. |
| `8` | Brake | Winds the old track down like a turntable stopping and drops the next one on the downbeat. |
| `9` | Auto | Picks one of the eight to suit the pair: see below. |

`[transitions] slots` in the config file changes what each key does and how long the blends
run. See [configuration](configuration.md).

## Pressing a key

| Press | What happens |
| --- | --- |
| One tap | Arms that transition. It waits for its downbeat, counting down in bars on the list. |
| The same key again, later | Disarms it. |
| Another key | Arms that one instead. |
| A quick double tap | Switches that key off. It shows `off` and greys out. Double tap it again to switch it back on. |
| `Esc` | Stops whatever is armed or running, and leaves every control where it is. |

Holding a key down does not count as a double tap. A double tap on a key also puts back
whatever was armed before its first tap.

## How a transition plans itself

**Where the mix starts.** On the outgoing deck, the first of these that is coming up:

- a hot cue in the second half of the track, within the next 32 bars, which counts as your
  mark for where to mix out;
- a change of section within the next 16 bars, such as the start of a breakdown or an outro;
- the next eight-bar phrase line, or four-bar for a cut.

A blend that would run past the end of the track is shortened to fit.

**Where the next track comes in.** Hot cue 1 if it is set, otherwise the main cue if you have
moved it, otherwise the first bar. A drop swap goes for the drop instead. A deck you have
already started is left where it is, with its beats pulled onto the outgoing deck's.

**What bends the plan.**

- The next track is brought to the outgoing tempo, folding half and double time. If the
  tempo fader cannot reach, a blend becomes a cut on the phrase line.
- Keys are compared on the Camelot wheel, at the pitch each track is actually playing at,
  unless key lock is holding it. When they clash, the mids swap over at once instead of being
  layered.
- Without a beat grid on both decks there are no beats to line up, so a blend becomes an
  eight-second fade with the lows kept apart.

A key whose plan had to bend shows its length in yellow. A key with nothing to do, such as
when nothing is playing or the other deck is empty, greys out.

**Auto** goes through it in this order: no grid, a fade; a tempo gap, a tempo bridge if the
two faders can close it between them and a cut if not; under 16 bars left, a loop out; a
breakdown coming up and a drop on the next track, a drop swap; clashing keys, a bass swap;
otherwise a long blend if there is room for one and a bass swap if not. With nothing armed,
the lines under the list say what Auto would pick.

## Sections

Bars and sections come from the low band of the overview waveform, read when a track loads.
A stretch with the kick in is the body, or a drop if it follows a breakdown. A stretch without
it is the intro, a breakdown, or the outro, depending on where it falls. A gap of less than
eight bars counts as a fill. The beat grid does not know which beat starts the bar, so that
is taken from the beat the low band changes on most often.

## Taking over by hand

Moving a control a transition is driving gives it back to you for the rest of that
transition: a fader or the crossfader, by key or by controller, or a kill switch on a band.
Everything else carries on.

## Volume

With the crossfader parked on the outgoing side, the transition rides the crossfader across.
Otherwise it rides the two channel faders with equal-power curves, ending the incoming fader
where the outgoing one was. At the end the old deck stops and both channels' EQ goes back to
flat.

## The EQ row

The transitions drive the EQ, so the mixer folds trim and EQ into one row. It reads `flat`
until something moves, then lights each band's letter: yellow for a cut, cyan for a boost,
red for a kill. A trim off zero shows beside the letters. `[ui] eq_rows = "always"` brings the
full rails back, for a controller with EQ knobs.

## Limits

- Steps run on the UI thread, at the frame rate. A cut can land up to one frame, about 33 ms,
  after its downbeat. The incoming track starts that much further in, so the beats still line
  up, but the cut itself is that late.
- Fader and EQ gains change in steps with no per-sample ramp, so a swap on a downbeat can
  click on loud material.
- The brake slows the deck in steps of one frame.
