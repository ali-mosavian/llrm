"""The llrm runtime's filled rows, lines and circles: LINE BF, long lines, CIRCLE, PAINT and a text scroll on the CGA and 256-colour screens, m16 and m32.

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
    1: {"box-filled": 1000, "paint": 3500, "print-scroll": 20000, "line-long": 2000, "circle": 950},
    13: {"box-filled": 600, "line-long": 1100},
}


@pytest.fixture(scope="module")
def timed(tmp_path_factory):
    if not qbruntime.dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    import bench  # noqa: E402

    return bench.run(list(LIMITS), tmp_path_factory.mktemp("gfxspeed"), only=("llrm m16", "llrm m32"))


@pytest.mark.parametrize("side", ["llrm m16", "llrm m32"])
@pytest.mark.parametrize("mode,segment", [(m, s) for m, limits in LIMITS.items() for s in limits])
def test_a_filled_row_costs_a_row_fill_not_a_pixel_loop(timed, side: str, mode: int, segment: str):
    assert timed[mode][segment][side] <= LIMITS[mode][segment]
