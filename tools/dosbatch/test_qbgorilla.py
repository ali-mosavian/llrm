"""GORILLA played to a hit, in real mode and on dos32: the first player's banana hits the second's gorilla, and the game
says so.

A banana that hit ended GORILLA with "Illegal function call": ExplodeGorilla's CIRCLE passes an aspect of -1.57, which the
runtime refused and BC takes (#1510).  No pixel test of CIRCLE saw it; a game played to the explosion does.

The game is the demo's source with RANDOMIZE (TIMER) made RANDOMIZE 1, so the skyline, the wind (-1) and the gorillas
(m16: left at x 141, y 223; right at x 449, y 147) are the same every time.  (75, 83) is the angle and velocity the banana's
arc, worked out from that screen with the game's own formulas (PlotShot), gets from the left gorilla into the right
one over the buildings, with the same hit for every gorilla position within a pixel of the estimate.  Skipped without
the demo sources (QB45_DEMOS_DIR) or QB45."""

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
import qbruntime  # noqa: E402

pytestmark = pytest.mark.slow_dos

# By target: m32's RND gives another sequence (#1512), so another skyline and another shot.
SHOT = {"16": ("75", "83"), "32": ("80", "104")}


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


def play_to_the_hit(exe: Path, loaders: tuple, work: Path, shot: tuple) -> str:
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
        session.type([*shot[0], "enter"], pause=0.3)
        time.sleep(1)
        session.type([*shot[1], "enter"], pause=0.3)
        session.wait_for("GAME OVER!", 60)
        return session.screen()
    finally:
        session.close()


@pytest.mark.parametrize("target", ["16", "32"])
def test_a_banana_that_hits_the_other_gorilla_scores_and_ends_the_game(game, tmp_path, target: str):
    exe, loaders = game[target]
    screen = play_to_the_hit(exe, loaders, tmp_path / "play", SHOT[target])
    assert "Illegal function call" not in screen
    assert re.search(r"Player 1\s+1\b", screen), screen
    assert re.search(r"Player 2\s+0\b", screen), screen
