"""The QB runtime's graphics against BCOM45: each tests/qbrt/gfx_*.bas runs to a SLEEP under DOSBox-X, once as
BCOM45 builds it and once with the llrm runtime, and the two screenshots must have no pixel that differs."""

from __future__ import annotations

import shutil
import sys
import time
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbplay  # noqa: E402

PROBES = sorted((dosbatch.ROOT / "tests" / "qbrt").glob("gfx_*.bas"))
SETTLE_SECONDS = 2
GRAPHICS_MODE = 0x10   # SCREEN 9: what the probes draw in


@pytest.fixture(scope="module")
def programs(tmp_path_factory):
    """Both builds of every probe, linked."""
    if not dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    return qbplay.built_pairs({source.stem: source for source in PROBES}, tmp_path_factory.mktemp("qbgfx"))


# Probes that go back to text before they wait: nothing to see change but the clock.
ENDS_IN_TEXT = {"gfx_back_to_text"}
TEXT_SECONDS = 6


def screenshot(exe: Path, work: Path, text: bool) -> Path:
    work.mkdir(exist_ok=True)
    shutil.copy(exe, work / "P.EXE")
    session = qbplay.Session(work)
    try:
        session.send({"cmd": "dos_cmd", "command": "P.EXE"}, wait=False)
        if text:
            time.sleep(TEXT_SECONDS)
        else:
            session.wait_for_mode(GRAPHICS_MODE)
            time.sleep(SETTLE_SECONDS)
        session.shot(work / "shot.png")
    finally:
        session.close()
    return work / "shot.png"


@pytest.mark.parametrize("name", [path.stem for path in PROBES])
def test_graphics_probe_has_no_pixel_different_from_bcom45(programs, tmp_path, name: str):
    reference, candidate = programs[name]
    want = screenshot(reference, tmp_path / "ref", name in ENDS_IN_TEXT)
    got = screenshot(candidate, tmp_path / "cand", name in ENDS_IN_TEXT)
    assert qbplay.differing_pixels(want, got) == 0
