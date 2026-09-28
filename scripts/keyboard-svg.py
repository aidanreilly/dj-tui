#!/usr/bin/env python3
"""Draw the dj-tui key map onto a UK ISO keyboard, as images/keyboard.svg.

The bindings here are the ones in crates/input/src/keymap/mix.rs. When a binding moves,
change it in both places and run this again:

    python3 scripts/keyboard-svg.py
"""

U = 62          # key unit
GAP = 6
PAD = 24
LEGEND_H = 92
R = 7           # corner radius

# Colour per group of controls, and what the swatches at the bottom say.
GROUPS = {
    "transport": ("#2f6f4f", "Transport"),
    "loop": ("#1f6f72", "Loops"),
    "mixer": ("#8a5a1f", "Mixer"),
    "browser": ("#27567f", "Browser"),
    "other": ("#4a4a52", "Everything else"),
}

# (legend, width in units, group, label, shift label)
# group None draws an unused key.
ROWS = [
    [
        ("`", 1, None, "", ""),
        ("1", 1, "transport", "hot cue 1", ""),
        ("2", 1, "transport", "hot cue 2", ""),
        ("3", 1, "transport", "hot cue 3", ""),
        ("4", 1, "transport", "hot cue 4", ""),
        ("5", 1, "transport", "hot cue 5", ""),
        ("6", 1, "transport", "hot cue 6", ""),
        ("7", 1, "transport", "hot cue 7", ""),
        ("8", 1, "transport", "hot cue 8", ""),
        ("9", 1, None, "", ""),
        ("0", 1, None, "", ""),
        ("-", 1, "mixer", "tempo dn", "both decks"),
        ("=", 1, "mixer", "tempo up", "both decks"),
        ("Backspace", 2, None, "", ""),
    ],
    [
        ("Tab", 1.5, "browser", "switch deck", ""),
        ("Q", 1, "transport", "quantize", ""),
        ("W", 1, "other", "waveform", ""),
        ("E", 1, None, "", ""),
        ("R", 1, "mixer", "trim", "R up"),
        ("T", 1, "mixer", "kill hi A", "T kill hi B"),
        ("Y", 1, "mixer", "kill mid A", "Y kill mid B"),
        ("U", 1, "mixer", "kill lo A", "U kill lo B"),
        ("I", 1, "loop", "loop in", "I loop out"),
        ("O", 1, "mixer", "filter LP", "O filter HP"),
        ("P", 1, None, "", ""),
        ("[", 1, "loop", "loop half", "{ fade len"),
        ("]", 1, "loop", "loop x2", "} fade len"),
    ],
    [
        ("Caps", 1.75, None, "", ""),
        ("A", 1, None, "", ""),
        ("S", 1, "transport", "sync", ""),
        ("D", 1, None, "", ""),
        ("F", 1, None, "", ""),
        ("G", 1, "transport", "seek 0-9", ""),
        ("H", 1, "mixer", "hp mix", "H master"),
        ("J", 1, "loop", "jump back", "J forward"),
        ("K", 1, "transport", "key lock", ""),
        ("L", 1, "loop", "loop 4 bt", ""),
        (";", 1, None, "", ""),
        ("'", 1, None, "", ""),
        ("#", 1, None, "", ""),
    ],
    [
        ("Shift", 1.25, None, "", ""),
        ("\\", 1, None, "", ""),
        ("Z", 1, None, "", ""),
        ("X", 1, "mixer", "xfade mid", "X slowly"),
        ("C", 1, "transport", "cue (hold)", ""),
        ("V", 1, "mixer", "filter mid", "V slowly"),
        ("B", 1, "browser", "browser", ""),
        ("N", 1, None, "", ""),
        ("M", 1, "mixer", "hp cue", ""),
        (",", 1, "mixer", "pitch dn", "< nudge bk"),
        (".", 1, "mixer", "pitch up", "> nudge fw"),
        ("/", 1, "browser", "search", "? help"),
        ("Shift", 2.75, None, "", ""),
    ],
    [
        ("Ctrl", 1.25, "other", "D device", "Q quit"),
        ("Win", 1.25, None, "", ""),
        ("Alt", 1.25, "other", "slowly", ""),
        ("Space", 6.25, "transport", "play / pause", ""),
        ("AltGr", 1.25, None, "", ""),
        ("Win", 1.25, None, "", ""),
        ("Menu", 1.25, None, "", ""),
        ("Ctrl", 1.25, None, "", ""),
    ],
]

# The arrow cluster and Esc, drawn to the right of the main block.
EXTRAS = [
    # (x units from block left, row, legend, width, group, label, shift label)
    (15.6, 0, "Esc", 1, "mixer", "stop fade", ""),
    (16.6, 3, "Up", 1, "mixer", "fader up", "end / fade"),
    (15.6, 4, "Left", 1, "mixer", "xfade to A", "end / fade"),
    (16.6, 4, "Down", 1, "mixer", "fader dn", "end / fade"),
    (17.6, 4, "Right", 1, "mixer", "xfade to B", "end / fade"),
]

BG = "#f6f6f8"
KEY_UNUSED = "#e3e3e8"
STROKE = "#c9c9d2"
INK = "#1b1b20"


def esc(t):
    return (
        t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    )


def key(x, y, w, legend, group, label, shift):
    fill = GROUPS[group][0] if group else KEY_UNUSED
    ink = "#ffffff" if group else "#8a8a95"
    out = [
        f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{U}" rx="{R}" '
        f'fill="{fill}" stroke="{STROKE}" stroke-width="1"/>'
    ]
    out.append(
        f'<text x="{x + 6:.1f}" y="{y + 16:.1f}" font-size="12" font-weight="600" '
        f'fill="{ink}" opacity="0.95">{esc(legend)}</text>'
    )
    if label:
        out.append(
            f'<text x="{x + w / 2:.1f}" y="{y + 36:.1f}" font-size="9.8" '
            f'text-anchor="middle" fill="#ffffff">{esc(label)}</text>'
        )
    if shift:
        out.append(
            f'<text x="{x + w / 2:.1f}" y="{y + 50:.1f}" font-size="9" '
            f'text-anchor="middle" fill="#ffffff" opacity="0.72">{esc(shift)}</text>'
        )
    return out


def main():
    block_w = 15 * U + 14 * GAP
    width = PAD * 2 + block_w + 3 * (U + GAP) + 24
    height = PAD * 2 + 5 * (U + GAP) + LEGEND_H
    body = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" font-family="Inter, Helvetica, Arial, sans-serif">',
        f'<rect width="{width}" height="{height}" fill="{BG}"/>',
        f'<text x="{PAD}" y="{PAD - 4}" font-size="13" font-weight="700" fill="{INK}">'
        f'dj-tui — mix mode on a UK keyboard</text>',
    ]
    top = PAD + 8
    for r, row in enumerate(ROWS):
        x = PAD
        y = top + r * (U + GAP)
        for legend, units, group, label, shift in row:
            w = units * U + (units - 1) * GAP
            body += key(x, y, w, legend, group, label, shift)
            x += w + GAP
    for ux, r, legend, units, group, label, shift in EXTRAS:
        w = units * U + (units - 1) * GAP
        body += key(PAD + ux * (U + GAP), top + r * (U + GAP), w, legend, group, label, shift)

    # The ISO Enter is the backwards-L spanning the top two letter rows, and it is the
    # key that loads a track, so it carries that rather than a duplicate beside the block.
    ly = top + (U + GAP)
    ex = PAD + 13.5 * (U + GAP)
    colour = GROUPS["browser"][0]
    body.append(
        f'<path d="M {ex:.1f} {ly:.1f} h {1.5 * U + 0.5 * GAP:.1f} v {2 * U + GAP:.1f} '
        f'h {-(1.25 * U + 0.25 * GAP):.1f} v {-U:.1f} h {-(0.25 * U + 0.25 * GAP):.1f} z" '
        f'fill="{colour}" stroke="{STROKE}"/>'
    )
    body.append(
        f'<text x="{ex + 10:.1f}" y="{ly + 16:.1f}" font-size="12" font-weight="600" '
        f'fill="#ffffff">Enter</text>'
    )
    body.append(
        f'<text x="{ex + 10:.1f}" y="{ly + 34:.1f}" font-size="10" fill="#ffffff">'
        f'load track</text>'
    )

    # Legend.
    ly = top + 5 * (U + GAP) + 18
    body.append(
        f'<text x="{PAD}" y="{ly:.1f}" font-size="11" fill="{INK}" opacity="0.75">'
        f'Shift is the second line on a key. Alt on a fader or the filter fades it instead of '
        f'stepping, and Alt on a hot cue clears it. Letters type into the browser while it '
        f'holds the keyboard, where Alt and a letter is its own command.</text>'
    )
    lx = PAD
    ly += 26
    for _, (colour, name) in GROUPS.items():
        body.append(
            f'<rect x="{lx}" y="{ly - 10:.1f}" width="13" height="13" rx="3" fill="{colour}"/>'
        )
        body.append(
            f'<text x="{lx + 19}" y="{ly:.1f}" font-size="11" fill="{INK}">{esc(name)}</text>'
        )
        lx += 26 + len(name) * 6.6 + 22
    body.append("</svg>")
    out = "\n".join(body) + "\n"
    with open("images/keyboard.svg", "w") as f:
        f.write(out)
    print(f"images/keyboard.svg: {len(out)} bytes, {width}x{height}")


if __name__ == "__main__":
    main()
