"""Which calls leave a runtime cell alone, and what that is worth.

qbdemo is the fixture because it is a program that sets DEF SEG and then
calls through loops: every call there used to end what b$seg held.
"""

from pathlib import Path

from tests import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.analysis import consts
from qbopt.objectfile.module import Space
from qbopt.frontend import raising_call_memory

FIXTURE = Path("fixtures/regressions/qbdemo-fil2.obj")


def _spared_names():
    found = corpus.loaded(FIXTURE)
    spared = raising_call_memory.spared(found, ir.decode_module(found), runtime.for_module(found))
    seen: dict[str, set[bool]] = {}
    for at, name in found.calls.items():
        excluded = any(addr.space is Space.EXTERNAL for addr, _ in spared.get(at, ()))
        seen.setdefault(name, set()).add(excluded)
    return seen


def test_a_call_that_cannot_write_b_seg_says_so() -> None:
    """B$INKY and B$SCMP sit in PLASMA's frame loop and ended b$seg.

    The routines that write it are the DEF SEG pair and CLEAR/RUN. UPDPALPLASMA
    only `out`s to the palette DAC, and PLASMA's `mov es,[b$seg]` stayed a load
    while its OUT was taken to write any byte.
    """
    seen = _spared_names()
    assert seen["B$INKY"] == {True} and seen["B$SCMP"] == {True} and seen["B$FIL2"] == {True}
    assert seen["B$DSEG"] == {False}, "DEF SEG writes it"
    assert seen["UPDPALPLASMA"] == {True}, "the DAC has no path to memory"


def _port_stores():
    found = corpus.loaded(FIXTURE)
    return {ir.span(node)[0]: node.effects.stores for body in ir.decode_module(found) for node in body.nodes}


def test_a_port_reaches_what_its_device_reaches() -> None:
    """A port is a device: the DAC's writes land in the palette, while the PIT
    at 0x40 is not one this names, so its read keeps unknown memory reach."""
    stores = _port_stores()
    assert all(stores[at] == () for at in (0x1529, 0x156C, 0x15AD, 0x15EC)), "UPDPALPLASMA's DAC writes"
    assert stores[0x13A9] == (), "WHITEFADE's DAC read"
    assert stores[0x1A3F] == ir.ANY_MEMORY, "BENCHSNAP's PIT read"


def test_one_selector_that_never_resolves_leaves_the_others_assumed() -> None:
    """All or nothing: a $DYNAMIC array's selector in the same body threw away
    the DEF SEG = &HA000 the POKE loop reads, and `mov es,[b$seg]` at 0x1103
    stayed a load.
    """
    assert _known_at(0x1103) == [consts.Known(0xA000, 2)]


def test_plasma_reads_the_def_seg_it_set() -> None:
    """Stage 5's case: PLASMA's `mov es,[b$seg]` at 0x0F28 stayed a load while
    UPDPALPLASMA's palette writes were taken to write b$seg."""
    assert _known_at(0x0F28) == [consts.Known(0xA000, 2)]


def _known_at(at: int) -> list:
    """What constant each value defined at `at` is known to hold."""
    found = corpus.loaded(FIXTURE)
    for _name, body in mir.bodies(found, corpus.partitioned(FIXTURE), runtime.for_module(found)):
        sites = [op for block in body.blocks for op in block.ops if op.at == at]
        if sites:
            known = consts.known(body, found.dgroup, found.calls)
            return [known.get(value) for value in sites[0].defines if not value.flags]
    raise AssertionError(f"{at:#x} is in no body")
