"""QGLDIFF reported off-exact 32 instead of 1: forwarding reused base+20 as base."""

from pathlib import Path

import corpus
from qbopt.analysis import avail
from qbopt.model import mir
from qbopt.optimize import transform


def test_plane_vertices_keep_distinct_addresses_after_forwarding():
    path = Path("fixtures/regressions/qrender-qgldiff-v-g3.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = next(body for name, body in mir.bodies(found, partition)
                if name == "procedure QGL_DIFF_PLANE")
    result = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    ops = {op.at: op for block in result.blocks for op in block.ops}
    loaded = next(arg for arg in ops[0x486].args if isinstance(arg, mir.Cell))
    subtracted = next(arg for arg in ops[0x492].args if isinstance(arg, mir.Cell))
    assert loaded != subtracted, "v(1).x - v(0).x became x - x"
    arithmetic = ops[0x480]
    assert avail.loaded_into(arithmetic) is None
