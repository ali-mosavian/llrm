"""Focused gates for residual BC register-pair recognition."""

from pathlib import Path

import pytest

from qbopt.model import mir
from qbopt.frontend import pairs
from qbopt.frontend import blocks as split
from qbopt.frontend.blocks import code_map

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def test_a_pair_needs_adjacent_addresses_and_a_known_pair() -> None:
    """The two facts that make two moves one long, and neither is optional."""
    from iced_x86 import Register

    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

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


def test_a_sign_extension_from_a_segment_register_is_not_a_long() -> None:
    """`mov ax,es / cwd` is the shape and not the meaning.

    mir.PHYSICAL keeps the segment registers out of the values, so a long
    seeded from one has a half nothing can account for. lift.py excludes it
    because es is not one of the registers it tracks; this excludes it for
    the reason underneath that.
    """
    from iced_x86 import Register
    from types import SimpleNamespace

    from qbopt.model import ir

    def extending(source: ir.Loc) -> tuple[mir.Op, mir.Op]:
        into = ir.Reg(register=Register.AX, width=2)
        low = mir.Op(
            at=0x100,
            op=ir.Operation.MOVE,
            name="mov",
            defines=(mir.Value(1, 0x100),),
            uses=(),
            node=SimpleNamespace(semantics=ir.Semantics(ir.Operation.MOVE, "mov", dests=(into,), sources=(source,))),
        )
        high = mir.Op(
            at=0x103,
            op=ir.Operation.NOTHING if hasattr(ir.Operation, "NOTHING") else ir.Operation.MOVE,
            name="cwd",
            defines=(mir.Value(2, 0x103),),
            uses=(mir.Value(1, 0x100),),
            node=SimpleNamespace(
                semantics=ir.Semantics(
                    ir.Operation.MOVE, "cwd", dests=(ir.Reg(register=Register.DX, width=2),), sources=()
                )
            ),
        )
        return low, high

    origin = {mir.Value(1, 0x100): Register.EAX, mir.Value(2, 0x103): Register.EDX}

    from_register = extending(ir.Reg(register=Register.BX, width=2))
    assert pairs._sign_extended(*from_register, origin) is not None, "an integer register widens"

    from_segment = extending(ir.Reg(register=Register.ES, width=2))
    assert pairs._sign_extended(*from_segment, origin) is None, "a segment register is not a value"


def test_two_negates_without_the_borrow_are_not_one_long_negate() -> None:
    """`neg ax / adc dx,0 / neg dx` -- the middle instruction is the negate.

    Negating a long is not negating each half: the low half's `neg` sets the
    borrow, and `adc dx,0` folds it into the high half before that is
    negated in turn. Two `neg`s on the two halves with anything else between
    them are two independent negates and mean something different.

    Nothing in the corpus has that near-miss, so removing the check changes
    no count and the object-by-object agreement with lift.py cannot see it.
    Built here instead.
    """
    from iced_x86 import Register
    from iced_x86 import Register_
    from types import SimpleNamespace

    from qbopt.model import ir

    def unary(at: int, name: str, register: Register_, value: int) -> mir.Op:
        where = ir.Reg(register=register, width=2)
        return mir.Op(
            at=at,
            op=ir.Operation.UNARY,
            name=name,
            defines=(mir.Value(value, at),),
            uses=(),
            node=SimpleNamespace(
                semantics=ir.Semantics(ir.Operation.UNARY, name, dests=(where,), sources=(where,))
            ),
        )

    low = unary(0x100, "neg", Register.AX, 1)
    high = unary(0x106, "neg", Register.DX, 3)
    origin = {mir.Value(1, 0x100): Register.EAX, mir.Value(2, 0x103): Register.EDX, mir.Value(3, 0x106): Register.EDX}

    borrow = mir.Op(
        at=0x103,
        op=ir.Operation.BINARY,
        name="adc",
        defines=(mir.Value(2, 0x103),),
        uses=(),
        node=SimpleNamespace(
            semantics=ir.Semantics(
                ir.Operation.BINARY,
                "adc",
                dests=(ir.Reg(register=Register.DX, width=2),),
                sources=(ir.Reg(register=Register.DX, width=2), ir.Imm(value=0, width=2)),
            )
        ),
    )
    assert pairs._negate([low, borrow, high], 0, origin) is not None, "the real idiom"

    unrelated = unary(0x103, "not", Register.DX, 2)
    assert pairs._negate([low, unrelated, high], 0, origin) is None, (
        "without the borrow folded in, these are two independent negates"
    )


@pytest.mark.parametrize("stem", ["negnot-q-O", "arith-v-g3"])
def test_raised_longs_leave_no_consumer_without_a_producer(stem: str) -> None:
    """negnot pushed a stale high word: every raised long consumer needs a producer."""
    from qbopt.objectfile import omf
    from qbopt.objectfile import module

    found = module.of(omf.parse((Path("fixtures/omf") / f"{stem}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    for _name, body in mir.bodies(found, blocks):
        made = {one.id for block in body.blocks for op in block.ops for one in op.defines}
        made |= {phi.result.id for block in body.blocks for phi in block.phis}
        entry = {one.id for block in body.blocks for op in block.ops for one in op.uses} - {
            one.id for block in body.blocks for op in block.ops for one in op.defines
        }
        orphans = [
            f"{op.at:#06x} reads {one}"
            for block in body.blocks
            for op in block.ops
            for one in op.uses
            if not one.flags and one.id not in made and one.id not in entry
        ]
        assert not orphans, "; ".join(orphans[:3])
