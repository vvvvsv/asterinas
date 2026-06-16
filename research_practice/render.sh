#!/bin/bash
# Regenerate all defense-deck diagrams and report their sizes in one step.
# Usage: bash render.sh   (no per-command approval needed once allow-listed)
set -e
cd "$(dirname "$0")"

echo "=== rendering (gen_diagrams.py) ==="
python3 gen_diagrams.py

echo
echo "=== output PNG sizes ==="
python3 - <<'PY'
from PIL import Image
import glob, os
for f in sorted(glob.glob('assets/*.png')):
    im = Image.open(f)
    w, h = im.size
    print(f"{os.path.basename(f):30s} {w:>4d}x{h:<4d}  ratio {w/h:.2f}")
PY
