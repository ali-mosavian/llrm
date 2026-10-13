"""The jpeg benches' input: a deterministic 256x192 picture, baseline 4:2:0 with a restart marker every 8 MCUs.

    python3 tools/jpeg_image.py bench/jpeg_single/photo.jpg
"""

import io
import sys
import math
from pathlib import Path

from PIL import Image, ImageDraw

WIDTH, HEIGHT = 256, 192
RESTART_MCUS = 8


def picture() -> Image.Image:
    """Gradient sky, a sun, hills and a noisy field: smooth areas, edges and texture, from integers only."""
    image = Image.new("RGB", (WIDTH, HEIGHT))
    pixels = image.load()
    seed = 12345
    for y in range(HEIGHT):
        for x in range(WIDTH):
            seed = (seed * 1103515245 + 12345) & 0x7FFFFFFF
            noise = (seed >> 16) % 24
            if y < 110:
                pixels[x, y] = (40 + y, 90 + y, 200 - y // 2)
            else:
                hill = int(20 * math.sin(x / 17.0))
                pixels[x, y] = (30 + noise, 110 + hill + noise, 40 + noise)
    draw = ImageDraw.Draw(image)
    draw.ellipse((180, 20, 230, 70), fill=(250, 220, 60))
    draw.polygon([(0, 110), (60, 50), (120, 110)], fill=(90, 80, 100))
    draw.rectangle((100, 120, 150, 170), outline=(255, 255, 255), width=3)
    draw.line((0, 191, 255, 100), fill=(200, 30, 30), width=2)
    return image


def encode() -> bytes:
    out = io.BytesIO()
    picture().save(out, "JPEG", quality=90, subsampling=2, restart_marker_blocks=RESTART_MCUS)
    return out.getvalue()


if __name__ == "__main__":
    Path(sys.argv[1]).write_bytes(encode())
