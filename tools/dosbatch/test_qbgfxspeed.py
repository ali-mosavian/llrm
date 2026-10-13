"""The llrm runtime's filled rows, lines and circles: LINE BF, long lines, CIRCLE, PAINT, GET and PUT and a text scroll on the CGA and 256-colour screens, m16 and m32.

Each ran a pixel at a time (a call, a read and a mask per pixel in the CGA modes): 150 boxes of 32x32 took 6,500 emulated ms
in SCREEN 1 and 1,300 in SCREEN 13, BCOM45's under 200; a scrolling PRINT took 33,800 in SCREEN 1.  The limits are about three times what a
row fill takes, so a return to per-pixel writes fails them."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))
sys.path.insert(0, str(Path(__file__).parent / "gfxbench"))

import qbruntime  # noqa: E402

LIMITS = {
    1: {"box-filled": 1000, "paint": 1500, "print-scroll": 1600, "print-short": 1600, "line-long": 700, "circle": 400, "get-put": 2500},
    2: {"print-short": 1500, "print-scroll": 1500},
    13: {"box-filled": 600, "line-long": 450, "get-put": 4000, "print-short": 1300},
}


@pytest.fixture(scope="module")
def timed(tmp_path_factory):
    if not qbruntime.dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    # Not `bench`: tools/bench has a module of that name, and one pytest process imports both.
    import gfxbench_run  # noqa: E402

    return gfxbench_run.run(list(LIMITS), tmp_path_factory.mktemp("gfxspeed"), only=("llrm real", "llrm flat"))


@pytest.mark.parametrize("mode,segment", [(m, s) for m, limits in LIMITS.items() for s in limits])
def test_a_filled_row_costs_a_row_fill_not_a_pixel_loop(timed, mode: int, segment: str):
    for side in ("llrm real", "llrm flat"):
        assert timed[mode][segment][side] <= LIMITS[mode][segment], side
