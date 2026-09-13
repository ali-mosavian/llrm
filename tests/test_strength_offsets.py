"""A loop's derived offsets advance instead of being recomputed from memory.

RENDER's DEF SEG loop added `[bp-38h]` and the array descriptor's offset to
its counter on every pass. Its one store goes through the literal selector
0A000h, and asked without constants that store could land on the frame and
on the descriptor, so neither load was invariant and nothing was reduced.
"""

from pathlib import Path

from qbopt import wholeseg
from qbopt.model import ir

FIXTURE = Path("fixtures/regressions/qbdemo-fil2.obj")


def _final(name: str):
    seen = {}

    def watch(stage, body_name, body):
        if body_name == f"procedure {name}" and stage == "peephole":
            seen["body"] = body

    wholeseg.emitted(FIXTURE.read_bytes(), watch=watch)
    return seen["body"]


def test_render_loads_nothing_but_its_source_pixel_inside_its_def_seg_loop() -> None:
    body = _final("RENDER")
    loop = [block for block in body.blocks if 0x10DE <= block.at < 0x1116]
    assert loop, "RENDER's inner loop is in no block"
    loads = [
        f"{one.at:#x} {one.what.name} {source}"
        for block in loop
        for one in block.insns
        if one.what is not None
        for source in one.what.sources
        if isinstance(source, ir.Mem) and source.selector is None
    ]
    assert not loads, loads


def test_render_steps_two_pointers_and_stores_its_counter_once() -> None:
    """The counter was stored to [bp-2Ch] and advanced beside both pointers
    on every pass; the pointers alone can end the loop."""
    from qbopt.backend import target

    body = _final("RENDER")
    loop = [block for block in body.blocks if 0x10DE <= block.at < 0x1116]
    stores = [
        f"{one.at:#x} {dest}"
        for block in loop
        for one in block.insns
        if one.what is not None
        for dest in one.what.dests
        if isinstance(dest, ir.Mem) and dest.selector is None
    ]
    assert not stores, stores
    written = {
        target.ir.ROOT.get(dest.register, dest.register)
        for block in loop
        for one in block.insns
        if one.what is not None
        for dest in one.what.dests
        if isinstance(dest, ir.Reg) and dest.register not in target.SEGMENTS
    }
    assert len(written) <= 3, written


def test_plasma_advances_its_array_offsets_instead_of_reloading_them() -> None:
    """PLASMA's x loop reloaded each descriptor's offset, [si+0Ah], every
    pass: forward left the counter's copy a load, so no counter was found.
    The third array's subscript is computed from pixel values, not from x."""
    body = _final("PLASMA")
    loop = [block for block in body.blocks if 0x0EE0 <= block.at < 0x0F3E]
    assert loop, "PLASMA's inner loop is in no block"
    offsets = [
        f"{one.at:#x} {source}"
        for block in loop
        for one in block.insns
        if one.what is not None
        for source in one.what.sources
        if isinstance(source, ir.Mem) and source.selector is None and source.offset == 0x0A
    ]
    assert len(offsets) <= 1, offsets


def test_render_reaches_both_arrays_through_its_counter() -> None:
    """The DEF SEG loop stepped a pointer per array beside its counter: three
    registers advanced or compared where one counter indexes both cells."""
    from qbopt.backend import target

    body = _final("RENDER")
    loop = [block for block in body.blocks if 0x10DE <= block.at < 0x1116]
    cells = [
        where
        for block in loop
        for one in block.insns
        if one.what is not None
        for where in (*one.what.dests, *one.what.sources)
        if isinstance(where, ir.Mem) and where.selector is not None
    ]
    assert len(cells) == 2 and all(cell.index_through != 0 for cell in cells), cells
    assert len({target.ir.ROOT.get(cell.index_through, cell.index_through) for cell in cells}) == 1, cells
    written = {
        target.ir.ROOT.get(dest.register, dest.register)
        for block in loop
        for one in block.insns
        if one.what is not None
        for dest in one.what.dests
        if isinstance(dest, ir.Reg) and dest.register not in target.SEGMENTS
    }
    assert len(written) <= 2, written


def test_render_counts_its_index_up_to_zero() -> None:
    """`inc edi; cmp di,9Fh; jle` compared every pass against a bound. Counting
    from -0A0h with both bases moved past their ends, the step's own flags end
    the loop: `inc edi; jnz`."""
    body = _final("RENDER")
    loop = [block for block in body.blocks if 0x10DE <= block.at < 0x1116]
    work = [
        f"{one.at:#x} {one.what.name}"
        for block in loop
        for one in block.insns
        if one.what is not None and one.what.op is not ir.Operation.NOTHING
    ]
    assert not any(line.endswith((" cmp", " test")) for line in work), work
    assert len(work) == 4, work


def test_render_branches_back_to_its_loop_without_a_jump_island() -> None:
    """Entered at its body, the loop's back edge became critical; its copies
    coalesced away but the split block stayed, a `jmp` placed after `retf`
    that every pass took."""
    body = _final("RENDER")
    (loop,) = [block for block in body.blocks if block.at == 0x10DE]
    branch = loop.insns[-1]
    assert branch.what.op is ir.Operation.BRANCH and branch.what.target == 0x10DE, branch.what
