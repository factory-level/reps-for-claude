#!/usr/bin/env python3
"""Draw the RFP brand marks from source, so the identity stays editable.

Everything is authored on a small pixel grid and scaled up with nearest-neighbour
resampling, which is what keeps the 16-bit look sharp at every size. Palette is
the app's own (app/src/retro.css) — do not introduce colours that are not here.

    python3 scripts/make-brand.py
    (cd app && pnpm tauri icon ../brand/icon.png)   # fan the icon out per platform
"""
from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[1]

INK = (74, 43, 71, 255)
IVORY = (250, 249, 245, 255)
ORANGE = (218, 119, 86, 255)
ORANGE_DEEP = (184, 92, 60, 255)

# 5x7 pixel glyphs. Only the characters the wordmark needs.
GLYPHS = {
    "R": ["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
    "F": ["11111", "10000", "10000", "11110", "10000", "10000", "10000"],
    "P": ["11110", "10001", "10001", "11110", "10000", "10000", "10000"],
    ">": ["11000", "01100", "00110", "00011", "00110", "01100", "11000"],
}


def box(img, x0, y0, x1, y1, colour):
    """Fill an inclusive pixel rectangle."""
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            img.putpixel((x, y), colour)


def mark() -> Image.Image:
    """32x32 barbell: ivory bar, orange plates, ink ground."""
    img = Image.new("RGBA", (32, 32), INK)
    box(img, 2, 15, 29, 16, IVORY)            # the bar, overhanging both collars
    for x0 in (3, 26):                        # collars
        box(img, x0, 10, x0 + 2, 21, ORANGE_DEEP)
    for x0 in (7, 21):                        # plates
        box(img, x0, 6, x0 + 3, 25, ORANGE)
    return img


def text(word: str, colour=IVORY) -> Image.Image:
    """Render `word` from the 5x7 glyphs with one pixel of letter spacing."""
    width = len(word) * 6 - 1
    img = Image.new("RGBA", (width, 7), (0, 0, 0, 0))
    for i, char in enumerate(word):
        for y, row in enumerate(GLYPHS[char]):
            for x, bit in enumerate(row):
                if bit == "1":
                    img.putpixel((i * 6 + x, y), colour)
    return img


def scale(img: Image.Image, factor: int) -> Image.Image:
    return img.resize((img.width * factor, img.height * factor), Image.NEAREST)


def wordmark() -> Image.Image:
    """`> RFP` — the caret carries the "prompts" half of the name."""
    caret, name = text(">", ORANGE), text("RFP")
    img = Image.new("RGBA", (caret.width + 3 + name.width, 7), (0, 0, 0, 0))
    img.paste(caret, (0, 0))
    img.paste(name, (caret.width + 3, 0))
    return img


def banner() -> Image.Image:
    """README header: mark and wordmark on ink, 3:1-ish."""
    img = Image.new("RGBA", (160, 48), INK)
    img.paste(mark(), (12, 8))
    word = scale(wordmark(), 2)
    img.paste(word, (56, (48 - word.height) // 2), word)
    return img


def write(img: Image.Image, relative: str, factor: int):
    out = REPO / relative
    out.parent.mkdir(parents=True, exist_ok=True)
    scale(img, factor).save(out)
    print(f"{relative}  {img.width * factor}x{img.height * factor}")


if __name__ == "__main__":
    write(mark(), "brand/icon.png", 32)           # 1024x1024 source of truth
    write(mark(), "web/app/icon.png", 8)          # Next.js app-router favicon
    write(mark(), "app/public/favicon.png", 8)
    write(wordmark(), "brand/wordmark.png", 12)
    write(banner(), "brand/banner.png", 8)        # 1280x384
