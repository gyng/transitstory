#!/usr/bin/env python3
"""Process the light-novel "subtitle" art into the title screen's transparent webp set.

Source: docs/subtitle/sub (N).png — gloriously over-the-top colour lettering on a SOLID BLACK
background. Output: packages/app/public/title/subN.webp — the same art with the black keyed to
transparency, trimmed to the lettering, and capped to a sane size, so each floats on the menu's
dark gradient (Menu.tsx renders a randomly-picked one as the hero).

Technique mirrors build_icon.py: remove the background via a BORDER-CONNECTED near-black flood
fill (so dark pixels INSIDE the lettering — outlines, shadows — are preserved, unlike a plain
threshold). Re-runnable + deterministic; REPLACES every existing subN.webp. Pure numpy + Pillow.

    python3 scripts/build_subtitles.py
"""
from __future__ import annotations

import pathlib
import re

import numpy as np
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "docs" / "subtitle"
OUT_DIR = ROOT / "packages" / "app" / "public" / "title"

THRESH = 44          # background = brightest channel <= this AND border-connected (matches build_icon)
MARGIN_FRAC = 0.02   # breathing room around the trimmed lettering
MAX_SIDE = 900       # longest output edge (px) — matches the shipped assets (retina-ish for the hero)
WEBP_QUALITY = 86


def dilate(mask: np.ndarray) -> np.ndarray:
    """4-connected binary dilation by one pixel (numpy, no scipy)."""
    out = mask.copy()
    out[1:, :] |= mask[:-1, :]
    out[:-1, :] |= mask[1:, :]
    out[:, 1:] |= mask[:, :-1]
    out[:, :-1] |= mask[:, 1:]
    return out


def background_mask(bright: np.ndarray, thresh: int) -> np.ndarray:
    """Border-connected region of near-black pixels = the true background (not interior darks)."""
    cand = bright <= thresh
    bg = np.zeros_like(cand)
    bg[0, :] |= cand[0, :]
    bg[-1, :] |= cand[-1, :]
    bg[:, 0] |= cand[:, 0]
    bg[:, -1] |= cand[:, -1]
    while True:
        grown = bg
        for _ in range(16):  # batch dilations to cut Python loop overhead
            grown = dilate(grown) & cand
        if np.array_equal(grown, bg):
            return bg
        bg = grown


def process(src_path: pathlib.Path) -> Image.Image:
    rgb = np.asarray(Image.open(src_path).convert("RGB")).astype(np.int16)
    bright = rgb.max(axis=2)
    bg = background_mask(bright, THRESH)
    subject = ~bg
    alpha = np.where(bg, 0, 255).astype(np.uint8)
    rgba = np.dstack([rgb.astype(np.uint8), alpha])

    ys, xs = np.where(subject)
    if len(xs) == 0:
        img = Image.fromarray(rgba, "RGBA")  # all background (shouldn't happen) — pass through
    else:
        h, w = bg.shape
        my = int((ys.max() - ys.min()) * MARGIN_FRAC)
        mx = int((xs.max() - xs.min()) * MARGIN_FRAC)
        y0, y1 = max(0, ys.min() - my), min(h, ys.max() + 1 + my)
        x0, x1 = max(0, xs.min() - mx), min(w, xs.max() + 1 + mx)
        img = Image.fromarray(rgba[y0:y1, x0:x1], "RGBA")

    w, h = img.size
    scale = MAX_SIDE / max(w, h)
    if scale < 1.0:
        img = img.resize((round(w * scale), round(h * scale)), Image.LANCZOS)
    return img


def src_index(p: pathlib.Path) -> int | None:
    m = re.search(r"\((\d+)\)", p.name)  # "sub (12).png" -> 12
    return int(m.group(1)) if m else None


def main() -> None:
    srcs = sorted(
        ((src_index(p), p) for p in SRC_DIR.glob("*.png") if src_index(p) is not None)
    )
    if not srcs:
        raise SystemExit(f"no 'sub (N).png' sources in {SRC_DIR}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for old in OUT_DIR.glob("sub*.webp"):  # replace ALL (leaves logo/mascot.webp untouched)
        old.unlink()

    for out_i, (_, p) in enumerate(srcs, start=1):
        img = process(p)
        out = OUT_DIR / f"sub{out_i}.webp"
        img.save(out, "WEBP", quality=WEBP_QUALITY, method=6)
        print(f"  sub{out_i}.webp  <- {p.name}  {img.size[0]}x{img.size[1]}  {out.stat().st_size // 1024}KB")
    print(f"[build_subtitles] wrote {len(srcs)} subtitles → {OUT_DIR}")
    print(f"[build_subtitles] set SUBTITLE_COUNT = {len(srcs)} in Menu.tsx")


if __name__ == "__main__":
    main()
