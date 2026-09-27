"""Turn the cell dumps and waveform images from `examples/screenshots.rs` into PNGs.

    python3 scripts/screenshots.py /tmp/shots

Writes <mode>.png (pixel waveforms, as Ghostty shows them) and <mode>_glyph.png (the
character fallback used in tmux or terminals without kitty graphics).
"""
import re, sys
from PIL import Image, ImageDraw, ImageFont

CW, CH = 10, 20
BG = (22, 22, 26)
FG = (220, 220, 220)
NAMED = {"Black": (0, 0, 0), "Red": (220, 50, 47), "Green": (80, 200, 80), "Yellow": (230, 200, 40),
         "Blue": (60, 110, 230), "Magenta": (200, 80, 200), "Cyan": (60, 200, 200), "Gray": (170, 170, 170),
         "DarkGray": (100, 100, 100), "White": (255, 255, 255)}
font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 16)
bold = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf", 16)

def colour(s, default):
    m = re.match(r"Rgb\((\d+), (\d+), (\d+)\)", s)
    if m:
        return tuple(int(v) for v in m.groups())
    return NAMED.get(s, default)

def render(mode, out, pixel):
    cells = [l.rstrip("\n").split("\t") for l in open(f"{out}/{mode}.cells", encoding="utf8")]
    cols = max(int(c[0]) for c in cells) + 1
    rows = max(int(c[1]) for c in cells) + 1
    img = Image.new("RGB", (cols * CW, rows * CH), BG)
    d = ImageDraw.Draw(img)
    for x, y, sym, fg, bg, mods in cells:
        x, y = int(x), int(y)
        f, b = colour(fg, FG), colour(bg, BG)
        if "REVERSED" in mods:
            f, b = b, f
        if "DIM" in mods:
            f = tuple(int(v * 0.5 + BG[i] * 0.5) for i, v in enumerate(f))
        d.rectangle([x * CW, y * CH, x * CW + CW - 1, y * CH + CH - 1], fill=b)
        if sym.strip():
            d.text((x * CW, y * CH + 1), sym, font=bold if "BOLD" in mods else font, fill=f)
    if pixel:
        for i in (0, 1):
            with open(f"{out}/{mode}_deck{i}.rgba", "rb") as fh:
                w, h, ax, ay = map(int, fh.readline().split())
                wave = Image.frombytes("RGBA", (w, h), fh.read())
            box = (ax * CW, ay * CH)
            img.paste(Image.new("RGB", (w, h), BG), box)
            img.paste(wave, box, wave)
    img.save(f"{out}/{mode}{'' if pixel else '_glyph'}.png")

out = sys.argv[1] if len(sys.argv) > 1 else "."
for mode in ("3band", "rgb", "blue"):
    render(mode, out, True)
render("3band", out, False)
