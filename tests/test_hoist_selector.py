"""A DEF SEG loop's descriptor loads, and the selector the POKE goes through."""

from pathlib import Path

from tests import corpus
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.optimize import transform

FIXTURE = Path("fixtures/regressions/qbdemo-fil2.obj")


def test_a_poke_through_a_constant_selector_lets_the_descriptor_load_leave() -> None:
    """RENDER's inner loop is `unf(scrx, scry) = PEEK(...)` under DEF SEG = &HA000.

    The hoist asked the alias question with the load's intervals only, and
    those knew loop counters and not that `es` held 0xa000, so the POKE could
    be in any segment and `mov es,[descriptor+2]` was reloaded every pixel.
    """
    found = corpus.loaded(FIXTURE)
    body = next(
        body
        for name, body in mir.bodies(found, corpus.partitioned(FIXTURE), runtime.for_module(found))
        if name == "procedure RENDER"
    )
    for one in ("fold", "decide", "lcssa", "hoist"):
        body = transform.applied(body, found.dgroup, found.calls, found=found, only=one)
    inner = next(block for block in body.blocks if any(op.at == 0x1107 and op.stores for op in block.ops))
    assert not any(op.at == 0x10F1 and op.loads for op in inner.ops)
