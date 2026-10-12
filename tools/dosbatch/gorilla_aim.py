"""Where a GORILLA banana lands, worked out from a screenshot of the game with the game's own formulas (PlotShot).

The screen of SCREEN 9 is the game's whole state: the buildings are the pixels that are not the sky, the gorillas are the
orange ones (OBJECTCOLOR), and the wind arrow along the bottom row has the wind in its length (WindLine = Wind * 3 * 2).
`shoot` is PlotShot's loop: the banana's position at t = 0, .1, .2 ..., and the two points POINT looks at.  Gravity is the
default 9.8.

    python3 gorilla_aim.py SCREENSHOT.png PLAYER [hit|building]   the angle and velocity that do
"""

from __future__ import annotations

import math
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import qbplay  # noqa: E402

ORANGE = (255, 170, 85)
SUN_YELLOW = (255, 255, 85)
GRAVITY = 9.8


class Scene:
    def __init__(self, path: Path):
        self.width, self.height, self.rgb = qbplay.png_pixels(path)
        self.sky = Counter(self.pixel(x, y) for y in range(self.height) for x in range(self.width)).most_common(1)[0][0]
        self.left, self.right = self.sprite(0, 320), self.sprite(320, self.width)
        row = [x for x in range(self.width) if self.pixel(x, self.height - 5) != self.sky]
        tip = min(row) if min(row) < 320 else max(row)
        self.wind = round((tip - 320) / 6) if row else 0

    def pixel(self, x: int, y: int) -> tuple[int, int, int]:
        if not (0 <= x < self.width and 0 <= y < self.height):
            return self.sky
        at = (y * self.width + x) * 3
        return tuple(self.rgb[at : at + 3])

    def sprite(self, x0: int, x1: int) -> tuple[int, int]:
        """The top left of the orange gorilla between two columns: GorillaX, GorillaY."""
        points = [(x, y) for y in range(self.height) for x in range(x0, x1) if self.pixel(x, y) == ORANGE]
        return min(p[0] for p in points), min(p[1] for p in points)

    def shoot(self, player: int, gorilla: tuple[int, int], angle: float, velocity: float):
        """('hit', colour, x, y) where the banana first meets something, else ('off',)."""
        radians = (angle if player == 1 else 180 - angle) / 180 * math.pi
        vx, vy = math.cos(radians) * velocity, math.sin(radians) * velocity
        x0 = gorilla[0] + (25 if player == 2 else 0)
        y0 = gorilla[1] - 4 - 3
        step = 4 if player == 2 else -4
        t = 0.0
        while t < 60:
            x = x0 + vx * t + 0.5 * (self.wind / 5) * t * t
            y = y0 + (-vy * t + 0.5 * GRAVITY * t * t)
            if x >= self.width - 10 or x <= 3 or y >= self.height - 3:
                return ("off",)
            if y > 0:
                look_x, look_y = 8 * (2 - player), 0
                while True:
                    colour = self.pixel(round(x + look_x), round(y + look_y))
                    sun = colour == SUN_YELLOW and 295 <= x + look_x <= 345 and y + look_y <= 45
                    impact = colour != self.sky and not sun
                    look_x, look_y = look_x + step, look_y + 6
                    if impact or look_x != 4:
                        break
                if impact:
                    return ("hit", colour, x, y)
            t += 0.1
        return ("off",)

    def aims(self, player: int, wanted: str) -> list[tuple[int, int]]:
        """Integer (angle, velocity) pairs that hit `wanted` ('hit': the other gorilla; 'building': not it), the same
        for a gorilla a pixel off in each direction."""
        mine, theirs = (self.left, self.right) if player == 1 else (self.right, self.left)
        found = []
        for angle in range(1, 90):
            for velocity in range(10, 201):
                good = True
                for dx in (-1, 0, 1):
                    for dy in (-1, 0, 1):
                        r = self.shoot(player, (mine[0] + dx, mine[1] + dy), angle, velocity)
                        far = r[0] == "hit" and (r[2] >= 320) == (player == 1)
                        if wanted == "hit" and not (r[0] == "hit" and r[1] == ORANGE and far):
                            good = False
                        if wanted == "building" and not (r[0] == "hit" and r[1] != ORANGE and 150 <= r[2] <= 490):
                            good = False
                    if not good:
                        break
                if good:
                    found.append((angle, velocity))
        return found


if __name__ == "__main__":
    scene = Scene(Path(sys.argv[1]))
    print("gorillas", scene.left, scene.right, "wind", scene.wind)
    found = scene.aims(int(sys.argv[2]), sys.argv[3] if len(sys.argv) > 3 else "hit")
    print(len(found), found[len(found) // 2] if found else None, found[:12])
