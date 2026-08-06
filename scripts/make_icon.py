#!/usr/bin/env python3
"""Generates the Harmonia master icon (1024x1024 PNG).

Design: a rounded square with a violet->teal gradient and three white
equalizer bars — a clean, modern mark that hints at audio without
relying on a generic note glyph.

Run:  python3 scripts/make_icon.py  ->  src-tauri/icons/icon-1024.png
"""
import os
from PIL import Image, ImageDraw

SIZE = 1024
TOP = (124, 92, 255)      # #7c5cff (default accent)
BOTTOM = (64, 200, 190)   # teal

def main() -> None:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))

    # Vertical gradient background.
    for y in range(SIZE):
        t = y / (SIZE - 1)
        color = tuple(int(a + (b - a) * t) for a, b in zip(TOP, BOTTOM))
        img.paste((*color, 255), (0, y, SIZE, y + 1))

    # Rounded-corner mask.
    mask = Image.new("L", (SIZE, SIZE), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, SIZE - 1, SIZE - 1], radius=224, fill=255
    )
    img.putalpha(mask)

    draw = ImageDraw.Draw(img)
    bar_w = 92
    # (center_x, top_y, bottom_y) — four bars of different heights.
    # PIL expects y0 <= y1, so top (smaller) comes first.
    bars = [(292, 360, 640), (462, 250, 640), (632, 470, 640), (802, 320, 640)]
    for cx, top, bottom in bars:
        draw.rounded_rectangle(
            [cx - bar_w // 2, top, cx + bar_w // 2, bottom],
            radius=46,
            fill=(255, 255, 255, 255),
        )

    out = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "icons", "icon-1024.png")
    img.save(os.path.abspath(out))
    print(f"wrote {os.path.abspath(out)}")


if __name__ == "__main__":
    main()
