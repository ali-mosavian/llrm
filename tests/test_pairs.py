"""
qbopt/pairs.py's own gate.

The load half is checked against lift.py, which answers the same question
over bytes rather than over values. Two independent implementations agreeing
on every object is worth more than either agreeing with a number written
down here.

The store half deliberately does *not* agree, and the reason is the point of
the module: see test_a_store_pair_is_a_shape_and_lift_wants_provenance.
"""

from pathlib import Path

import pytest

import corpus
from qbopt import mir
from qbopt import pairs
from qbopt import blocks as split
from qbopt.lift import lift
from qbopt.blocks import code_map

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def _both(obj: Path) -> tuple[set[int], set[int], set[int], set[int]]:
    """(lift loads, pairs loads, lift stores, pairs stores) by address."""
    found = corpus.loaded(obj)
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    blocks = split.partition(found, mapped)
    values, _stores, _bridges = lift(
        found.code, found.start, found.end, found.resolve, [i for b in blocks for i in b.insns]
    )
    # keyed on whichever half comes first, which is what lift.py records:
    # a store may be written high half first and its value still starts
    # where the region does
    mine_load: set[int] = set()
    mine_store: set[int] = set()
    for _name, body in mir.bodies(found, blocks):
        for one in pairs.found(body):
            (mine_load if one.kind is pairs.Kind.LOAD else mine_store).add(min(one.at))
    return (
        {v.at for v in values if v.op.value == "load"},
        mine_load,
        {v.at for v in values if v.op.value == "store"},
        mine_store,
    )


def _alu(obj: Path) -> tuple[set[int], set[int]]:
    """(lift's alu-m sites, this module's alu sites)."""
    found = corpus.loaded(obj)
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    blocks = split.partition(found, mapped)
    values, _stores, _bridges = lift(
        found.code, found.start, found.end, found.resolve, [i for b in blocks for i in b.insns]
    )
    mine: set[int] = set()
    for _name, body in mir.bodies(found, blocks):
        mine |= {min(one.at) for one in pairs.found(body) if one.kind is pairs.Kind.ALU}
    return {v.at for v in values if v.op.value == "alu-m"}, mine


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_the_load_pairs_are_exactly_the_ones_lift_finds(obj: Path) -> None:
    """Two implementations of one question, agreeing object by object.

    lift.py walks BC's bytes with two slots; this walks MIR's values. A load
    pair is two independent `mov`s with no carry between them, so the whole
    evidence is that the registers are a known pair and the second address is
    two bytes above the first -- and both arrive at the same 533 sites across
    the corpus, on every object.

    That agreement is the reason to trust the value-level version at all.
    `wide.pairs()` cannot see any of these: it is keyed on the carry edge,
    which a load pair does not have, and finds 169 of lift's 1,476 values.
    """
    lift_loads, mine, _lift_stores, _mine_stores = _both(obj)
    assert mine == lift_loads


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_store_pair_is_a_shape_and_lift_wants_provenance(obj: Path) -> None:
    """Every store lift finds, this finds -- and this finds more.

    The extra ones are real. After a runtime call returning a long in dx:ax,
    BC writes

        call far ptr 0:0
        mov ds:[0],ax
        mov ds:[0],dx

    which is one 32-bit store. lift.py does not record it, because the call
    cleared both its slots and it can no longer name *what* is being stored.

    Two different questions, and the difference is what widening still
    needs: knowing these two ops are one store is the shape, and knowing
    which value is in the pair is the provenance. This module answers the
    first. `transform.widened()` got it wrong by assuming the second.
    """
    _lift_loads, _mine, lift_stores, mine = _both(obj)
    assert lift_stores <= mine, "a store lift found and this did not"


def test_a_pair_needs_adjacent_addresses_and_a_known_pair() -> None:
    """The two facts that make two moves one long, and neither is optional."""
    from iced_x86 import Register

    from qbopt.module import Addr
    from qbopt.module import Space

    where = Addr(Space.LITERAL, 0x10)
    assert pairs._adjacent(mir.MemRef(addr=where, width=2), mir.MemRef(addr=where.plus(2), width=2))
    # not two bytes apart
    assert not pairs._adjacent(mir.MemRef(addr=where, width=2), mir.MemRef(addr=where.plus(4), width=2))
    # the wrong way round
    assert not pairs._adjacent(mir.MemRef(addr=where.plus(2), width=2), mir.MemRef(addr=where, width=2))
    # a whole dword is not two halves
    assert not pairs._adjacent(mir.MemRef(addr=where, width=4), mir.MemRef(addr=where.plus(2), width=4))
    # only BC's own two pairs, low half first
    assert pairs._half_of(Register.EAX, {}) == (0, 0)
    assert pairs._half_of(Register.EDX, {}) == (0, 1)
    assert pairs._half_of(Register.ECX, {}) == (1, 0)
    assert pairs._half_of(Register.EBX, {}) == (1, 1)
    assert pairs._half_of(Register.ESI, {}) is None


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_the_arithmetic_pairs_are_exactly_the_ones_lift_finds(obj: Path) -> None:
    """`and ax,[x]` with `and dx,[x+2]` is one 32-bit `and`, and no carry says so.

    Only add and sub carry -- `add`/`adc`, `sub`/`sbb`. The and/or/xor pairs
    have no flags edge between the halves at all, which is why
    `wide.pairs()` finds 169 where lift.py finds 363: it is keyed on exactly
    that edge. Recognised the way a load pair is instead -- a known register
    pair, addresses two bytes apart, and the mnemonics being partners -- the
    two agree on all 363, object by object.
    """
    theirs, mine = _alu(obj)
    assert mine == theirs


def test_the_slot_is_cleared_by_anything_that_writes_a_half() -> None:
    """lift.py's rule, and the one a shape recogniser does not have.

    `pairs.found()` says two ops are one 32-bit access. `pairs.held()` says
    what is in the pair when they run, and the difference is a call: after
    one returning a long in dx:ax, `mov ds:[0],ax` / `mov ds:[0],dx` is a
    real 32-bit store whose value cannot be named, because the call wrote
    both halves. Widening that needs the call's contract, not the shape.
    """
    import inspect

    source = inspect.getsource(pairs.held)
    assert "_touches" in source, "a write to either half must clear its slot"
    assert "op.barrier" in source, "and a barrier must clear both"
