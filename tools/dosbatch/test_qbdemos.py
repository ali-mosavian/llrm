"""NIBBLES and GORILLA played with typed keys under DOSBox-X, as BCOM45 builds them and with the llrm runtime:
at each point where the screen holds nothing random the two screenshots must agree, but for the blink of the
cursor.  What a game draws from RND (the skyline, the sparkles, where a number lies) is not compared."""

from __future__ import annotations

import shutil
import sys
import time
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbplay  # noqa: E402
import qbruntime  # noqa: E402

CURSOR_PIXELS = 32   # the cursor blinks: a few pixels of difference at most

# (keys to type, seconds to wait, checkpoint name or None, text the screen must show first or None) for
# each demo.
SCRIPTS = {
    "NIBBLES": [
        ([], 7, None, "Press any key"),
        (["space"], 3, "players", "How many players"),
        (["1", "enter"], 2, None, "Skill level"),
        (["1", "enter"], 2, None, "Increase game speed"),
        (["n", "enter"], 2, "monochrome", "Monochrome or color"),
        (["c", "enter"], 1, "level", "Push Space"),
    ],
    "GORILLA": [
        ([], 7, None, "Press any key"),
        (["space"], 3, "names", "Name of Player 1"),
        (["A", "l", "enter"], 1.5, "first", "Name of Player 2"),
        (["enter"], 1.5, "second", "Play to how many"),
        (["1", "enter"], 2, "points", None),
        (["enter"], 4, "ready", None),
    ],
}


@pytest.fixture(scope="module")
def demos(tmp_path_factory):
    found, reason = qbruntime.demo_sources()
    if not found or not dosbatch.QB45.is_dir():
        pytest.skip(reason or "QB45_DIR is unavailable")
    sources = {path.stem.upper(): path for path in found.values()}
    return qbplay.built_pairs(sources, tmp_path_factory.mktemp("qbdemos"))


def play(exe: Path, work: Path, script) -> dict[str, Path]:
    work.mkdir(exist_ok=True)
    shutil.copy(exe, work / "P.EXE")
    shots: dict[str, Path] = {}
    session = qbplay.Session(work)
    try:
        session.send({"cmd": "dos_cmd", "command": "P.EXE"}, wait=False)
        for keys, seconds, name, shown in script:
            session.type(keys)
            if shown:
                session.wait_for(shown, 60)
            time.sleep(seconds)
            if name:
                shots[name] = work / f"{name}.png"
                session.shot(shots[name])
    finally:
        session.close()
    return shots


@pytest.mark.parametrize("name", sorted(SCRIPTS))
def test_the_demo_shows_what_bcom45_shows_where_nothing_is_random(demos, tmp_path, name: str):
    reference, candidate = demos[name]
    want = play(reference, tmp_path / "ref", SCRIPTS[name])
    got = play(candidate, tmp_path / "cand", SCRIPTS[name])
    for checkpoint in want:
        assert qbplay.differing_pixels(want[checkpoint], got[checkpoint]) <= CURSOR_PIXELS, checkpoint
