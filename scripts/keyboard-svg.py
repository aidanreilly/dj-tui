#!/usr/bin/env python3
"""Draw the dj-tui key maps onto a UK ISO keyboard.

Two layers, because the keyboard has two: images/keyboard.svg is mix mode and
images/keyboard-browser.svg is browser mode, where letters type into the filter and every
command sits on Alt. The bindings here are the ones in crates/input/src/keymap/mix.rs and
crates/input/src/keymap/browser.rs. When a binding moves, change it in both places and run
this again:

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
    "filter": ("#6f3f6f", "Filters"),
    "other": ("#4a4a52", "Everything else"),
}

# (legend, width in units, group, label, shift label)
# group None draws an unused key.
MIX_ROWS = [
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
        ("T", 1, "mixer", "kill lo A", "T kill lo B"),
        ("Y", 1, "mixer", "kill mid A", "Y kill mid B"),
        ("U", 1, "mixer", "kill hi A", "U kill hi B"),
        ("I", 1, "loop", "loop in", "I loop out"),
        ("O", 1, "mixer", "master LP", "O master HP"),
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
        ("V", 1, "mixer", "master mid", "V slowly"),
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
MIX_EXTRAS = [
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


# Browser mode. Letters are letters here: they type into the filter, so every command sits
# on Alt and the only plain keys that do anything are the ones a text field already owns.
# `filter` is the group for the keys that narrow the list, which is what this mode is for.
BROWSER_ROWS = [
    [
        ("`", 1, "other", "types", ""),
        ("1", 1, "other", "types", ""),
        ("2", 1, "other", "types", ""),
        ("3", 1, "other", "types", ""),
        ("4", 1, "other", "types", ""),
        ("5", 1, "other", "types", ""),
        ("6", 1, "other", "types", ""),
        ("7", 1, "other", "types", ""),
        ("8", 1, "other", "types", ""),
        ("9", 1, "other", "types", ""),
        ("0", 1, "other", "types", ""),
        ("-", 1, "other", "types", ""),
        ("=", 1, "other", "types", ""),
        ("Backspace", 2, "browser", "rub out", ""),
    ],
    [
        ("Tab", 1.5, "browser", "switch deck", ""),
        ("Q", 1, "other", "types", ""),
        ("W", 1, "other", "types", ""),
        ("E", 1, "other", "types", ""),
        ("R", 1, "other", "types", ""),
        ("T", 1, "other", "types", ""),
        ("Y", 1, "other", "types", ""),
        ("U", 1, "browser", "Ctrl clear", ""),
        ("I", 1, "other", "types", ""),
        ("O", 1, "other", "types", ""),
        ("P", 1, "other", "types", ""),
        ("[", 1, "other", "types", ""),
        ("]", 1, "other", "types", ""),
    ],
    [
        ("Caps", 1.75, None, "", ""),
        ("A", 1, "browser", "Alt read", "and analyse"),
        ("S", 1, "browser", "Alt sort", "Alt+S flip"),
        ("D", 1, "filter", "Alt look up", "genre"),
        ("F", 1, "browser", "Alt size", ""),
        ("G", 1, "filter", "Alt genre", ""),
        ("H", 1, "other", "types", ""),
        ("J", 1, "other", "types", ""),
        ("K", 1, "filter", "Alt key", "that mix"),
        ("L", 1, "other", "types", ""),
        (";", 1, "other", "types", ""),
        ("'", 1, "other", "types", ""),
        ("#", 1, "other", "types", ""),
    ],
    [
        ("Shift", 1.25, None, "", ""),
        ("\\", 1, "other", "types", ""),
        ("Z", 1, "other", "types", ""),
        ("X", 1, "other", "types", ""),
        ("C", 1, "other", "types", ""),
        ("V", 1, "other", "types", ""),
        ("B", 1, "filter", "Alt tempo", "window"),
        ("N", 1, "other", "types", ""),
        ("M", 1, "other", "types", ""),
        (",", 1, "other", "types", ""),
        (".", 1, "other", "types", ""),
        ("/", 1, "other", "types", ""),
        ("Shift", 2.75, None, "", ""),
    ],
    [
        ("Ctrl", 1.25, None, "", ""),
        ("Win", 1.25, None, "", ""),
        ("Alt", 1.25, "browser", "commands", ""),
        ("Space", 6.25, "other", "types a space", ""),
        ("AltGr", 1.25, None, "", ""),
        ("Win", 1.25, None, "", ""),
        ("Menu", 1.25, None, "", ""),
        ("Ctrl", 1.25, None, "", ""),
    ],
]

BROWSER_EXTRAS = [
    (15.6, 0, "Esc", 1, "browser", "back to mix", "keeps the filter"),
    (16.6, 3, "Up", 1, "browser", "up the list", ""),
    (16.6, 4, "Down", 1, "browser", "down the list", ""),
]


def draw(rows, extras, title, note, enter_label, path):
    block_w = 15 * U + 14 * GAP
    width = PAD * 2 + block_w + 3 * (U + GAP) + 24
    height = PAD * 2 + 5 * (U + GAP) + LEGEND_H
    body = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" font-family="Inter, Helvetica, Arial, sans-serif">',
        f'<rect width="{width}" height="{height}" fill="{BG}"/>',
        f'<text x="{PAD}" y="{PAD - 4}" font-size="13" font-weight="700" fill="{INK}">'
        f'{esc(title)}</text>',
    ]
    top = PAD + 8
    for r, row in enumerate(rows):
        x = PAD
        y = top + r * (U + GAP)
        for legend, units, group, label, shift in row:
            w = units * U + (units - 1) * GAP
            body += key(x, y, w, legend, group, label, shift)
            x += w + GAP
    for ux, r, legend, units, group, label, shift in extras:
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
        f'{esc(enter_label)}</text>'
    )

    # Legend.
    ly = top + 5 * (U + GAP) + 18
    body.append(
        f'<text x="{PAD}" y="{ly:.1f}" font-size="11" fill="{INK}" opacity="0.75">'
        f'{esc(note)}</text>'
    )
    lx = PAD
    ly += 26
    used = {g for row in rows for (_, _, g, _, _) in row if g}
    used |= {g for (_, _, _, _, g, _, _) in extras if g}
    for name_key, (colour, name) in GROUPS.items():
        if name_key not in used:
            continue
        body.append(
            f'<rect x="{lx}" y="{ly - 10:.1f}" width="13" height="13" rx="3" fill="{colour}"/>'
        )
        body.append(
            f'<text x="{lx + 19}" y="{ly:.1f}" font-size="11" fill="{INK}">{esc(name)}</text>'
        )
        lx += 26 + len(name) * 6.6 + 22
    body.append("</svg>")
    out = "\n".join(body) + "\n"
    with open(path, "w") as f:
        f.write(out)
    print(f"{path}: {len(out)} bytes, {width}x{height}")


def main():
    draw(
        MIX_ROWS,
        MIX_EXTRAS,
        "dj-tui \u2014 mix mode on a UK keyboard",
        "Shift is the second line. Alt on a fader or the filter fades it instead of "
        "stepping, and Alt on a hot cue clears it.",
        "load track",
        "images/keyboard.svg",
    )
    draw(
        BROWSER_ROWS,
        BROWSER_EXTRAS,
        "dj-tui \u2014 browser mode on a UK keyboard",
        "Letters type into the filter, so every command is on Alt. Type bpm:124-128, "
        "key:9a or genre:house to filter by a field, anything else searches the name.",
        "load onto the focused deck",
        "images/keyboard-browser.svg",
    )


if __name__ == "__main__":
    main()
