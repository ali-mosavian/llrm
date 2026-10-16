"""The QB runtime's graphics against BCOM45: each tests/qbrt/gfx_*.bas runs to a SLEEP under DOSBox-X, once as
BCOM45 builds it and once with the llrm runtime, and the two screenshots must have no pixel that differs."""

from __future__ import annotations

import shutil
import sys
from pathlib import Path

import pytest

# BCOM45 (BC, LINK and its runtime) is the oracle these tests compare against: the gate drops the file where QB45 is not installed.
REQUIRES = ["qb45"]

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32demos  # noqa: E402
import qbplay  # noqa: E402

PROBES = sorted(path for path in (dosbatch.ROOT / "tests" / "qbrt").glob("gfx_*.bas") if ".flat" not in path.name)
pytestmark = pytest.mark.slow_dos

GRAPHICS_MODE = 0x10   # SCREEN 9: what the probes draw in, unless the name says
BIOS_MODES = {"gfx_getput1": 4, "gfx_getput2": 6, "gfx_getput7": 0x0D, "gfx_getput13": 0x13, "gfx_screen7": 0x0D, "gfx_screen8": 0x0E, "gfx_screen12": 0x12, "gfx_screen13": 0x13, "gfx_screen1": 4, "gfx_screen1_default": 4, "gfx_text1": 4, "gfx_text2": 6, "gfx_text13": 0x13, "gfx_text12": 0x12, "gfx_text8": 0x0E, "gfx_screen2": 6}


class Lazy:
    """Both builds of a probe, linked, made when a test asks: a run of some of the probes (the gate deals them over processes) builds only those."""

    def __init__(self, factory):
        self.factory, self.made = factory, {}

    def __getitem__(self, name: str):
        if name not in self.made:
            self.made[name] = qplay_built(name, self.factory.mktemp("qbgfx"))
        return self.made[name]


def qplay_built(name: str, work: Path):
    return qbplay.built({name: next(path for path in PROBES if path.stem == name)}, work)[name]


@pytest.fixture(scope="module")
def programs(tmp_path_factory):
    if not dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    return Lazy(tmp_path_factory)


# Probes that go back to text before they wait: nothing to see change but the clock.
ENDS_IN_TEXT = {"gfx_back_to_text"}


def screenshot(exe: Path, work: Path, text: bool, mode: int = GRAPHICS_MODE, files: tuple = ()) -> Path:
    work.mkdir(exist_ok=True)
    shutil.copy(exe, work / "P.EXE")
    for beside in files:
        shutil.copy(beside, work / Path(beside).name)
    session = qbplay.Session(work)
    try:
        session.launch(qb32demos.LONGEST_MS)
        if text:
            session.run_until(lambda: "red" in session.screen(), qb32demos.LONGEST_MS, "'red'")  # the last thing gfx_back_to_text prints
        else:
            session.run_until(lambda: session.video_mode() == mode, qb32demos.LONGEST_MS, f"BIOS mode {mode:#x}")
        # The probe has drawn and sleeps (SLEEP asks DOS whether a key is there, which is no keyboard read the emulator can park at):
        # the picture stops changing.
        qb32demos.settled(session, "the probe", None, looks=5, look_ms=100)
        session.snap(work / "shot.png")
    finally:
        session.close()
    return work / "shot.png"


@pytest.mark.parametrize("name", [path.stem for path in PROBES])
def test_graphics_probe_has_no_pixel_different_from_bcom45(programs, tmp_path, name: str):
    """BCOM45's picture is the oracle; ours, at m16 and at m32, must have no pixel that differs from it."""
    reference, candidates = programs[name]
    mode = BIOS_MODES.get(name, GRAPHICS_MODE)
    want = screenshot(reference, tmp_path / "ref", name in ENDS_IN_TEXT, mode)
    for target, built in candidates.items():
        got = screenshot(built.exe, tmp_path / f"cand{target}", name in ENDS_IN_TEXT, mode, built.files)
        assert qbplay.differing_pixels(want, got) == 0, f"m{target}"
