#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 DigitGram
"""The Windows icons (assets/*.ico) from the PNG icons of NfoGlow for Linux, which sits next to this folder.
Run it after the design changed there: python3 tools/make_icons.py (needs Pillow)."""
import shutil
import subprocess
import tempfile
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent.parent
ICONS = HERE.parent / "nfoglow" / "data" / "icons" / "hicolor"
SIZES = [(16, 16), (20, 20), (24, 24), (32, 32), (40, 40), (48, 48), (64, 64), (128, 128), (256, 256)]

for source, target in (("512x512/apps/io.github.digitgram.NfoGlow.png", "nfoglow.ico"),
                       ("128x128/mimetypes/io.github.digitgram.NfoGlow-nfo.png", "nfo-file.ico")):
    img = Image.open(ICONS / source).convert("RGBA")
    # Pillow drops ICO sizes larger than the source image, and Explorer's "large icons" need the 256 one.
    # So the file icon is rendered from the SVG at 256 px when resvg is installed (cargo install resvg),
    # otherwise the 128 px PNG is scaled up.
    if img.width < 256:
        svg = ICONS / "scalable/mimetypes/io.github.digitgram.NfoGlow-nfo.svg"
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "big.png"
            if shutil.which("resvg") and subprocess.run(["resvg", "-w", "256", "-h", "256", str(svg), str(out)]).returncode == 0:
                img = Image.open(out).convert("RGBA")
            else:
                img = img.resize((256, 256), Image.LANCZOS)
    img.save(HERE / "assets" / target, sizes=SIZES)
# the window icon (eframe shows it in the title bar and the taskbar)
Image.open(ICONS / "256x256/apps/io.github.digitgram.NfoGlow.png").save(HERE / "assets" / "nfoglow-256.png")
