# Waveforms

Three colour modes. `w` cycles them, and `[ui] waveform_mode` sets the one the app starts in.

| Mode | What you see |
| --- | --- |
| 3-Band | Blue lows, amber mids and white highs layered as on a CDJ-3000. |
| RGB | One blended colour per column: red lows, green mids, blue highs. |
| Blue | A single blue waveform that brightens toward white as the highs rise. |

Where the terminal supports the kitty graphics protocol, which covers Ghostty, kitty and
WezTerm, the waveforms are drawn as real pixels. Everywhere else they fall back to block
characters. `[ui] graphics = "off"` forces the block characters.

Clicking a waveform seeks there.
