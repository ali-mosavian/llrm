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
Both targets' RND give BC's sequence (#1512), so one skyline and the same shots.  Skipped without the demo sources
(QB45_DEMOS_DIR)."""

from __future__ import annotations

import re
import shutil
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import qb32demos  # noqa: E402
import qbbuild  # noqa: E402
import qbplay  # noqa: E402
import gorilla_aim  # noqa: E402
import qbruntime  # noqa: E402

pytestmark = pytest.mark.slow_dos

WAITING = 3   # empty keyboard reads in a row, picture unchanged, before a prompt is taken to be asking

# By target: the gorillas (GorillaX, GorillaY) and the wind of the seeded game, then the shots (angle, velocity).
SCENES = {target: ((141, 223), (449, 147), -1) for target in qbbuild.TARGETS}
HIT = {target: [("75", "83")] for target in qbbuild.TARGETS}
# The first lands on a building (m16: x 290, y 158; m32: x 250, y 123), the second is the other player's, into the gorilla.
BUILDING = {target: [("73", "56"), ("83", "102")] for target in qbbuild.TARGETS}


@pytest.fixture(scope="module")
def game(tmp_path_factory):
    found, reason = qbruntime.demo_sources()
    if not found:
        pytest.skip(reason or "the demo sources are unavailable")
    work = tmp_path_factory.mktemp("gorilla")
    original = {path.stem.upper(): path for path in found.values()}["GORILLA"]
    text = original.read_text()
    assert "RANDOMIZE (TIMER)" in text
    seeded = work / "GORSEED.BAS"
    seeded.write_text(text.replace("RANDOMIZE (TIMER)", "RANDOMIZE 1"))
    return qbbuild.build(seeded, work / "build", qbbuild.runtimes(work))


def play(built: qbbuild.Built, work: Path, scene: tuple, shots: list) -> str:
    """The game to its end with `shots` typed in turn; the screen at the end."""
    work.mkdir()
    shutil.copy(built.exe, work / "P.EXE")
    for beside in built.files:
        shutil.copy(beside, work / Path(beside).name)
    session = qbplay.Session(work)
    try:
        session.launch(qb32demos.LONGEST_MS)
        qb32demos.ready(session, "Press any key", typed=False)
        # the intro, the names (Player 1, Player 2), the points, the gravity, then P to play
        for keys in (["space"], ["enter"], ["enter"], ["1", "enter"], ["enter"], ["p"]):
            session.keys_in_lockstep(keys, qb32demos.LONGEST_MS)
        session.run_until(lambda: session.video_mode() == 0x10, qb32demos.LONGEST_MS, "BIOS mode 10h (SCREEN 9)")
        qb32demos.settled(session, "skyline", None)
        session.snap(work / "skyline.png")
        seen = gorilla_aim.Scene(work / "skyline.png")
        assert (seen.left, seen.right, seen.wind) == scene, "the seeded game is not the one the shots were worked out for: run gorilla_aim.py"
        for angle, velocity in shots:
            session.keys_in_lockstep([*angle, "enter", *velocity, "enter"], qb32demos.LONGEST_MS, waiting=WAITING)  # the banana, the explosion, the next prompt
        session.run_until(lambda: "GAME OVER!" in session.screen(), qb32demos.LONGEST_MS, "'GAME OVER!'")
        return session.screen()
    finally:
        session.close()


def scores(screen: str) -> tuple[int, int]:
    first, second = re.search(r"Player 1\s+(\d+)", screen), re.search(r"Player 2\s+(\d+)", screen)
    assert first and second, screen
    return int(first.group(1)), int(second.group(1))


def test_a_banana_that_hits_the_other_gorilla_scores_and_ends_the_game(game, tmp_path):
    for target in qbbuild.TARGETS:
        screen = play(game[target], tmp_path / f"hit{target}", SCENES[target], HIT[target])
        assert "Illegal function call" not in screen, f"{target}"
        assert scores(screen) == (1, 0), f"{target}: {screen}"


def test_a_banana_that_hits_a_building_scores_nothing_and_the_turn_passes(game, tmp_path):
    for target in qbbuild.TARGETS:
        screen = play(game[target], tmp_path / f"building{target}", SCENES[target], BUILDING[target])
        assert "Illegal function call" not in screen, f"{target}"
        assert scores(screen) == (0, 1), f"{target}: {screen}"
