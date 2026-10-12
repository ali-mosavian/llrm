"""GORILLA played with aimed shots, in real mode and on dos32.

A banana that hit a gorilla ended the game with "Illegal function call": ExplodeGorilla's CIRCLE passes an aspect of -1.57,
which the runtime refused and BC takes (#1510).  No pixel test of CIRCLE saw it; a game played to the explosion does.

Two cases, each on both targets.  A gorilla hit: the first player's banana goes into the second's gorilla (only a gorilla
hit scores), the game ends with "GAME OVER!" and the first player at 1.  A building hit, which is a different explosion
(DoExplosion): the first player's banana lands on a building, nothing scores, the turn passes, and the second player's
banana hits the first's gorilla, so the game ends with the second at 1 and the first at 0.

The game is the demo's source with RANDOMIZE (TIMER) made RANDOMIZE 1, so the skyline, the wind and the gorillas are the
same every time; the test checks the screen has them before it shoots.  The shots are worked out from the screen with the
game's own formulas (gorilla_aim.py, PlotShot's): each hits for every gorilla position within a pixel of the estimate.
m32's RND gives another sequence (#1512), so another skyline and other shots.  Skipped without the demo sources
(QB45_DEMOS_DIR) or QB45."""

from __future__ import annotations

import re
import shutil
import sys
import time
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qbplay  # noqa: E402
import gorilla_aim  # noqa: E402
import qbruntime  # noqa: E402

pytestmark = pytest.mark.slow_dos

# By target: the gorillas (GorillaX, GorillaY) and the wind of the seeded game, then the shots (angle, velocity).
SCENES = {"16": ((141, 223), (449, 147), -1), "32": ((71, 248), (559, 198), 3)}
HIT = {"16": [("75", "83")], "32": [("80", "104")]}
# The first lands on a building (m16: x 290, y 158; m32: x 250, y 123), the second is the other player's, into the gorilla.
BUILDING = {"16": [("73", "56"), ("83", "102")], "32": [("35", "182"), ("63", "81")]}


@pytest.fixture(scope="module")
def game(tmp_path_factory):
    found, reason = qbruntime.demo_sources()
    if not found or not dosbatch.QB45.is_dir():
        pytest.skip(reason or "QB45_DIR is unavailable")
    work = tmp_path_factory.mktemp("gorilla")
    original = {path.stem.upper(): path for path in found.values()}["GORILLA"]
    text = original.read_text()
    assert "RANDOMIZE (TIMER)" in text
    seeded = work / "GORSEED.BAS"
    seeded.write_text(text.replace("RANDOMIZE (TIMER)", "RANDOMIZE 1"))
    _, real = qbplay.built_pairs({"GORSEED": seeded}, work)["GORSEED"]
    flat = work / "flat"
    flat.mkdir()
    runtime = qb32.build(flat / "rt")
    obj = flat / "g.obj"
    done = dosbatch.subprocess.run(
        [str(dosbatch.BIN / "llrm-qb"), str(seeded), "--dialect", "qb45", *qbruntime.frontend_flags(seeded), "-O2", "-march=i486", dosbatch.m_flag(qb32.TARGET), "-o", str(obj)],
        capture_output=True,
        text=True,
    )
    assert done.returncode == 0, done.stderr[-300:]
    exe = flat / "GORSEED.EXE"
    loaders = qb32.link(obj, runtime, exe, flat)
    return {"16": (real, ()), "32": (exe, tuple(loaders))}


def play(exe: Path, loaders: tuple, work: Path, scene: tuple, shots: list) -> str:
    """The game to its end with `shots` typed in turn; the screen at the end."""
    work.mkdir()
    shutil.copy(exe, work / "P.EXE")
    for loader in loaders:
        shutil.copy(loader, work / Path(loader).name)
    session = qbplay.Session(work)
    try:
        session.send({"cmd": "dos_cmd", "command": "P.EXE"}, wait=False)
        session.wait_for("Press any key", 60)
        time.sleep(1)
        for keys, wait in ((["space"], 3), (["enter"], 1), (["enter"], 1), (["1", "enter"], 1.5), (["enter"], 2.5)):
            session.type(keys, pause=0.3)
            time.sleep(wait)
        session.type(["p"], pause=0.3)
        session.wait_for_mode(0x10, 60)  # BIOS mode 10h is SCREEN 9
        time.sleep(3)
        session.shot(work / "skyline.png")
        seen = gorilla_aim.Scene(work / "skyline.png")
        assert (seen.left, seen.right, seen.wind) == scene, "the seeded game is not the one the shots were worked out for: run gorilla_aim.py"
        for at, (angle, velocity) in enumerate(shots):
            session.type([*angle, "enter"], pause=0.3)
            time.sleep(1)
            session.type([*velocity, "enter"], pause=0.3)
            if at + 1 < len(shots):
                time.sleep(10)  # the banana, the explosion, the other player's prompt
        session.wait_for("GAME OVER!", 60)
        return session.screen()
    finally:
        session.close()


def scores(screen: str) -> tuple[int, int]:
    first, second = re.search(r"Player 1\s+(\d+)", screen), re.search(r"Player 2\s+(\d+)", screen)
    assert first and second, screen
    return int(first.group(1)), int(second.group(1))


@pytest.mark.parametrize("target", ["16", "32"])
def test_a_banana_that_hits_the_other_gorilla_scores_and_ends_the_game(game, tmp_path, target: str):
    exe, loaders = game[target]
    screen = play(exe, loaders, tmp_path / "play", SCENES[target], HIT[target])
    assert "Illegal function call" not in screen
    assert scores(screen) == (1, 0), screen


@pytest.mark.parametrize("target", ["16", "32"])
def test_a_banana_that_hits_a_building_scores_nothing_and_the_turn_passes(game, tmp_path, target: str):
    exe, loaders = game[target]
    screen = play(exe, loaders, tmp_path / "play", SCENES[target], BUILDING[target])
    assert "Illegal function call" not in screen
    assert scores(screen) == (0, 1), screen
