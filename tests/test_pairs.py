"""Focused gates for residual BC register-pair recognition."""

from pathlib import Path

import pytest

import corpus
from qbopt.model import mir
from qbopt.frontend import pairs
from qbopt.frontend import blocks as split
from qbopt.frontend.blocks import code_map

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def test_widened_restore_reads_the_value_it_splits() -> None:
    """nbody widening changed PX0 from 1258 to 47624: restores had no input dependency."""
    from qbopt.model import ir

    value = mir.Value(1, 0, 0, 1, 1)
    after = mir.Op(0, ir.Operation.MOVE, "mov", (value,), (), (), (), None,
                   kind=mir.Kind.COPY, results=(mir.Held(value, 4),))
    restored = pairs._restore_op(0, 3, after, 7)
    assert restored.uses == (value,)
    from dataclasses import replace
    store = replace(after, defines=(), results=(), args=(mir.Held(value, 4),))
    assert pairs._restore_op(0, 3, store, 7).uses == (value,)


@pytest.mark.parametrize("number", [0, 1])
def test_restore_clobbers_its_high_half_even_when_dead(number: int) -> None:
    """nbody printed PX0=1163 for 1258 when a restore overwrote its array index."""
    from qbopt.model import ir
    from qbopt.backend import lower

    value = mir.Value(1, 0)
    after = mir.Op(0, ir.Operation.MOVE, "mov", (value,), (),
                   kind=mir.Kind.COPY, results=(mir.Held(value, 4),))
    restored = pairs._restore_op(number, 3, after, 7)
    source, high = mir.RESTORE_PAIR[number]
    body = mir.MirBody(0, (mir.MirBlock(0, (), (after, restored), ()),), {value: source})
    low = lower.lowered("restore", body, {}, set(), {})
    assert high in low.blocks[0].insns[-1].clobbers
    assert source not in low.blocks[0].insns[-1].clobbers


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


def test_the_slot_is_cleared_by_anything_that_writes_a_half() -> None:
    """lift.py's rule, and the one a shape recogniser does not have.

    `pairs.found()` says two ops are one 32-bit access. `pairs.held()` says
    what is in the pair when they run, and the difference is a call: after
    one returning a long in dx:ax, `mov ds:[0],ax` / `mov ds:[0],dx` is a
    real 32-bit store whose value cannot be named, because the call wrote
    both halves. Widening that needs the call's contract, not the shape.
    """
    found = corpus.loaded(Path("fixtures/omf/arith-p-g2-zd.obj"))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)

    cleared = False
    for _name, body in mir.bodies(found, split.partition(found, mapped)):
        state = pairs.held(body)
        for block in body.blocks:
            live = {0: False, 1: False}
            for op in block.ops:
                slots = state[op.at]
                for number in pairs.PAIRS:
                    if live[number] and slots[number] is None:
                        cleared = True
                    live[number] = slots[number] is not None
    assert cleared, "no slot was ever cleared, so nothing here proves a write to a half stops the pair being known"


def test_a_pair_doubled_is_recognised_and_not_chained() -> None:
    """`add ax,ax / adc dx,dx` is a real 32-bit add of a pair with itself.

    lift.py does not report it as alu-v; this does, which is the recogniser
    being broader rather than wrong. What stops it mattering is that
    held() will not chain from a slot it does not know, and the one site in
    the corpus is exactly that case -- so the shape is counted and the
    provenance is still refused.
    """
    from qbopt.frontend import declen

    at = 0x12D
    found = corpus.loaded(Path("fixtures/omf/procs-p-evt.obj"))
    assert found is not None
    low = declen.decode(found.code, at)
    assert low is not None and str(low.insn) == "add ax,ax"
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    for _name, body in mir.bodies(found, split.partition(found, mapped)):
        state = pairs.held(body)
        for one in pairs.found(body):
            if min(one.at) == at:
                assert one.kind is pairs.Kind.ALU_REG
                assert state[at][one.pair] is None, "a doubling of an unknown pair stays unknown"
                return


def test_a_sign_extension_from_a_segment_register_is_not_a_long() -> None:
    """`mov ax,es / cwd` is the shape and not the meaning.

    mir.PHYSICAL keeps the segment registers out of the values, so a long
    seeded from one has a half nothing can account for. lift.py excludes it
    because es is not one of the registers it tracks; this excludes it for
    the reason underneath that.
    """
    from iced_x86 import Register

    from qbopt.model import ir

    def extending(source: ir.Loc) -> tuple[mir.Op, mir.Op]:
        into = ir.Reg(register=Register.AX, width=2)
        low = mir.Op(
            at=0x100,
            op=ir.Operation.MOVE,
            name="mov",
            defines=(mir.Value(1, 0x100),),
            uses=(),
            made=ir.Semantics(ir.Operation.MOVE, "mov", dests=(into,), sources=(source,)),
        )
        high = mir.Op(
            at=0x103,
            op=ir.Operation.NOTHING if hasattr(ir.Operation, "NOTHING") else ir.Operation.MOVE,
            name="cwd",
            defines=(mir.Value(2, 0x103),),
            uses=(mir.Value(1, 0x100),),
            made=ir.Semantics(ir.Operation.MOVE, "cwd", dests=(ir.Reg(register=Register.DX, width=2),), sources=()),
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

    from qbopt.model import ir

    def unary(at: int, name: str, register: Register_, value: int) -> mir.Op:
        where = ir.Reg(register=register, width=2)
        return mir.Op(
            at=at,
            op=ir.Operation.UNARY,
            name=name,
            defines=(mir.Value(value, at),),
            uses=(),
            made=ir.Semantics(ir.Operation.UNARY, name, dests=(where,), sources=(where,)),
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
        made=ir.Semantics(
            ir.Operation.BINARY,
            "adc",
            dests=(ir.Reg(register=Register.DX, width=2),),
            sources=(ir.Reg(register=Register.DX, width=2), ir.Imm(value=0, width=2)),
        ),
    )
    assert pairs._negate([low, borrow, high], 0, origin) is not None, "the real idiom"

    unrelated = unary(0x103, "not", Register.DX, 2)
    assert pairs._negate([low, unrelated, high], 0, origin) is None, (
        "without the borrow folded in, these are two independent negates"
    )


@pytest.mark.parametrize("stem", ["negnot-q-O", "arith-v-g3"])
def test_widening_leaves_no_consumer_without_a_producer(stem: str) -> None:
    """negnot pushed a stale high word: `push dx` read v3_5, and widening
    had removed the `neg dx` that defined it without putting anything in
    its place. The restore is what hands the pair back, and it said it
    defined nothing -- so the value had no interval, the allocator skipped
    it, and its pin was never applied.
    """
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.optimize import transform

    found = module.of(omf.parse((Path("fixtures/omf") / f"{stem}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    for _name, body in mir.bodies(found, blocks):
        wide = transform.widened(body)
        made = {one.id for block in wide.blocks for op in block.ops for one in op.defines}
        made |= {phi.result.id for block in wide.blocks for phi in wide.blocks[0].phis} if False else set()
        made |= {phi.result.id for block in wide.blocks for phi in block.phis}
        entry = {one.id for block in body.blocks for op in block.ops for one in op.uses} - {
            one.id for block in body.blocks for op in block.ops for one in op.defines
        }
        orphans = [
            f"{op.at:#06x} reads {one}"
            for block in wide.blocks
            for op in block.ops
            for one in op.uses
            if not one.flags and one.id not in made and one.id not in entry
        ]
        assert not orphans, "; ".join(orphans[:3])
