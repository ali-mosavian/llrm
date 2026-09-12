"""A selector that does not change stays in a segment register.

A 386 has ES, FS and GS. RENDER's DEF SEG loop reads its source array through
one selector and writes the screen through another, and with ES as the only
segment register the loop reloaded ES from a spill slot and pushed 0A000h
into it on every pass.
"""

from pathlib import Path

from qbopt import wholeseg
from qbopt.model import ir
from qbopt.backend import target

FIXTURE = Path("fixtures/regressions/qbdemo-fil2.obj")


def _final(name: str):
    seen = {}

    def watch(stage, body_name, body):
        if body_name == f"procedure {name}" and stage == "peephole":
            seen["body"] = body

    wholeseg.emitted(FIXTURE.read_bytes(), watch=watch)
    return seen["body"]


def test_render_loads_no_selector_inside_its_def_seg_loop() -> None:
    body = _final("RENDER")
    loop = [block for block in body.blocks if 0x10DE <= block.at < 0x1116]
    assert loop, "RENDER's inner loop is in no block"
    writes = [
        one
        for block in loop
        for one in block.insns
        if one.what is not None
        and any(isinstance(dest, ir.Reg) and dest.register in target.SEGMENTS for dest in one.what.dests)
    ]
    assert not writes, [f"{one.at:#x} {one.what}" for one in writes]
