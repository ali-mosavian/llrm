"""An update the program writes in place stays one instruction.

x86 adds to memory directly. qbdemo's RENDER does `add word ptr [bp-2Eh],0FF60h`
after comparing the same cell, and forwarding the compare's load into the add
turned it into a load, an add and a store.
"""

from pathlib import Path

from qbopt import wholeseg
from qbopt.model import ir
from qbopt.objectfile.module import Space

FIXTURE = Path("fixtures/regressions/qbdemo-fil2.obj")


def _final(name: str):
    seen = {}

    def watch(stage, body_name, body):
        if body_name == f"procedure {name}" and stage == "peephole":
            seen["body"] = body

    wholeseg.emitted(FIXTURE.read_bytes(), watch=watch)
    return seen["body"]


def _frame_cell(where, disp: int) -> bool:
    return (
        isinstance(where, ir.Mem)
        and where.addr is not None
        and where.addr.space is Space.FRAME
        and where.addr.disp == disp
    )


def test_render_adds_to_its_accumulator_in_place() -> None:
    body = _final("RENDER")
    in_place = [
        one
        for one in body.insns
        if one.what is not None
        and one.what.name == "add"
        and one.what.dests
        and _frame_cell(one.what.dests[0], -0x2E)
        and one.what.sources[-1:] == (ir.Imm(-160, 2),)
    ]
    assert in_place, "add word [bp-2Eh],0FF60h was split into load, add and store"


def test_render_counts_its_column_without_reloading_it() -> None:
    """RENDER's column loop reloaded `[bp-2Ch]` every pass: the header stores
    it, but the screen store through `es` = 0xA000 was taken to reach the
    frame, so availability forgot the value the header had just written."""
    seen = {}

    def watch(stage, body_name, body):
        if body_name == "procedure RENDER" and stage == "mir-r01-forward":
            seen.setdefault("body", body)

    wholeseg.emitted(FIXTURE.read_bytes(), watch=watch)
    reloads = [
        op
        for block in seen["body"].blocks
        for op in block.ops
        if op.at == 0x110A and any(ref.addr is not None and ref.addr.disp == -0x2C for ref in op.loads)
    ]
    assert not reloads
