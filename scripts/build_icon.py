#!/usr/bin/env python3
"""Build the app favicon set from the raw mascot art.

Takes the conductor-pigeon illustration (`docs/icon-raw.png`, an opaque RGB image
on a solid black background), removes the black background via a border-connected
flood fill (so interior dark areas — the navy cap, gold badge, dark feather bands,
eyes — are preserved), trims the surrounding margins to the subject, pads to a
centered square, and emits a transparent master plus the favicon set served from
`packages/app/public/`.

Pure stdlib + numpy + Pillow (no ImageMagick / scipy dependency). Re-run with:

    python3 scripts/build_icon.py

Deterministic: same input -> same outputs.
"""
from __future__ import annotations

import pathlib

import numpy as np
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "docs" / "icon-raw.png"
PUBLIC = ROOT / "packages" / "app" / "public"
MASTER = ROOT / "docs" / "icon.png"  # cleaned art reused by the README

# Background = pixels whose brightest channel is <= THRESH *and* that are
# connected to the image border. Tuned against the navy cap (brighter than this
# at its core) so the flood never reaches into the subject. See docs review.
THRESH = 44
MARGIN_FRAC = 0.03  # breathing room around the subject in the square canvas

# Favicon raster sizes. .ico bundles the small trio; the PNGs cover modern
# <link rel> hints (general favicon, Android, Apple touch).
ICO_SIZES = [16, 32, 48]
PNG_SIZES = {
    "favicon-16.png": 16,
    "favicon-32.png": 32,
    "favicon-48.png": 48,
    "apple-touch-icon.png": 180,
    "icon-192.png": 192,
    "icon-512.png": 512,
}


def dilate(mask: np.ndarray) -> np.ndarray:
    """4-connected binary dilation by one pixel (numpy, no scipy)."""
    out = mask.copy()
    out[1:, :] |= mask[:-1, :]
    out[:-1, :] |= mask[1:, :]
    out[:, 1:] |= mask[:, :-1]
    out[:, :-1] |= mask[:, 1:]
    return out


def background_mask(bright: np.ndarray, thresh: int) -> np.ndarray:
    """Border-connected region of near-black pixels = the true background."""
    cand = bright <= thresh
    bg = np.zeros_like(cand)
    bg[0, :] |= cand[0, :]
    bg[-1, :] |= cand[-1, :]
    bg[:, 0] |= cand[:, 0]
    bg[:, -1] |= cand[:, -1]
    # Grow the border seed inside the candidate set until it stops changing.
    while True:
        grown = bg
        for _ in range(16):  # batch dilations to cut Python loop overhead
            grown = dilate(grown) & cand
        if np.array_equal(grown, bg):
            return bg
        bg = grown


def build_rgba() -> Image.Image:
    rgb = np.asarray(Image.open(SRC).convert("RGB")).astype(np.int16)
    bright = rgb.max(axis=2)
    bg = background_mask(bright, THRESH)
    subject = ~bg

    alpha = np.where(subject, 255, 0).astype(np.float32)
    # Feather only the cut edge: average alpha in a 1px band so downscaled
    # favicons read smooth, while the interior stays fully opaque.
    edge = subject & dilate(bg)
    box = (
        alpha
        + np.pad(alpha[1:, :], ((0, 1), (0, 0)))
        + np.pad(alpha[:-1, :], ((1, 0), (0, 0)))
        + np.pad(alpha[:, 1:], ((0, 0), (0, 1)))
        + np.pad(alpha[:, :-1], ((0, 0), (1, 0)))
    ) / 5.0
    alpha = np.where(edge, box, alpha)

    rgba = np.dstack([rgb.astype(np.uint8), alpha.astype(np.uint8)])
    img = Image.fromarray(rgba, "RGBA")

    # Trim to the subject's bounding box, then center on a padded square so the
    # taller-than-wide bird is not distorted when squashed into icon frames.
    ys, xs = np.where(subject)
    img = img.crop((int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1))
    side = max(img.size)
    canvas = side + 2 * round(side * MARGIN_FRAC)
    square = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    square.paste(img, ((canvas - img.width) // 2, (canvas - img.height) // 2), img)
    return square


def main() -> None:
    PUBLIC.mkdir(parents=True, exist_ok=True)
    master = build_rgba()
    master.save(MASTER)
    print(f"wrote {MASTER.relative_to(ROOT)}  {master.size}")

    for name, size in PNG_SIZES.items():
        master.resize((size, size), Image.Resampling.LANCZOS).save(PUBLIC / name)
        print(f"wrote {(PUBLIC / name).relative_to(ROOT)}  {size}x{size}")

    master.save(PUBLIC / "favicon.ico", sizes=[(s, s) for s in ICO_SIZES])
    print(f"wrote {(PUBLIC / 'favicon.ico').relative_to(ROOT)}  {ICO_SIZES}")


if __name__ == "__main__":
    main()
