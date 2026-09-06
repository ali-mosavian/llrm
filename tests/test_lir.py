"""What each operation requires of a register."""

from pathlib import Path

import pytest
from iced_x86 import Register

from qbopt import ir
from qbopt import lir
from qbopt import mir
from qbopt import target
from qbopt import omf
from qbopt import module
from qbopt import blocks as split
from qbopt.blocks import code_map

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def _semantics(op: mir.Op) -> ir.Semantics | None:
    return op.made if op.made is not None else getattr(op.node, "semantics", None)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_bcs_own_assignment_satisfies_every_requirement(obj: Path) -> None:
    """The table is only worth having if the code it describes obeys it.

    BC put every value somewhere, and if this says an instruction needs one
    in a register BC did not use for it, the requirement is wrong. Two of
    them were: `bp` was left out of the addressing class, which made every
    `[bp-12h]` in the corpus a violation, and `fdivp` is modelled as a
    DIVIDE, so the widening rule claimed it reads dx:ax when there is
    nothing in ax to read.
    """
    found = module.of(omf.parse(obj.read_bytes()))
    if found is None:
        return
    mapped = code_map(found)
    if isinstance(mapped, str):
        return

    for name, body in mir.bodies(found, split.partition(found, mapped)):
        for block in body.blocks:
            for op in block.ops:
                what = _semantics(op)
                if what is None:
                    continue
                held = {ir.ROOT.get(body.origin.get(one, -1), -1) for one in op.uses}
                for want, need in target.reads(what).items():
                    if need.fixed is not None:
                        assert want in held, (
                            f"{obj.stem} {name}: {op.at:#x} {op.name} is said to need "
                            f"{want} and BC has no value there"
                        )
                    else:
                        assert want in {ir.ROOT.get(one, one) for one in need.where}, (
                            f"{obj.stem} {name}: {op.at:#x} {op.name} reaches memory by "
                            f"{want}, which the class does not permit"
                        )


def test_the_widening_forms_need_what_they_do_not_name() -> None:
    """`imul word [k]` multiplies by ax and says so nowhere.

    One destination written down is `imul r,r/m`, which names everything it
    touches. More than one is dx:ax, which names neither.
    """
    cell = ir.Mem(None, 2)
    ax = ir.Reg(register=Register.AX, width=2)
    dx = ir.Reg(register=Register.DX, width=2)

    named = ir.Semantics(ir.Operation.MULTIPLY, "imul", dests=(ax,), sources=(ax, cell))
    assert not any(need.fixed for need in target.reads(named).values()), "this one names its operands"

    wide = ir.Semantics(ir.Operation.MULTIPLY, "imul", dests=(ax, dx), sources=(cell,))
    assert target.reads(wide)[Register.EAX].fixed is Register.EAX
    assert target.writes(wide)[Register.EDX].fixed is Register.EDX

    # A divide reads both halves of the dividend.
    divide = ir.Semantics(ir.Operation.DIVIDE, "idiv", dests=(ax, dx), sources=(cell,))
    assert set(target.reads(divide)) == {Register.EAX, Register.EDX}

    # And x87 shares none of it, however `ir` happens to model the op.
    on_stack = ir.Semantics(ir.Operation.DIVIDE, "fdivp", dests=(ir.St(0),), sources=(ir.St(1),))
    assert target.reads(on_stack) == {} and target.writes(on_stack) == {}


def test_a_shift_by_a_register_takes_its_count_in_cl() -> None:
    """The one place a count may live, and the instruction does not say it."""
    ax = ir.Reg(register=Register.AX, width=2)
    cl = ir.Reg(register=Register.CL, width=1)

    by_one = ir.Semantics(ir.Operation.BINARY, "shl", dests=(ax,), sources=(ax, ir.Imm(value=1, width=1)))
    assert not any(need.fixed for need in target.reads(by_one).values())

    by_cl = ir.Semantics(ir.Operation.BINARY, "shl", dests=(ax,), sources=(ax, cl))
    assert target.reads(by_cl)[Register.ECX].fixed is Register.ECX


def test_the_addressing_class_is_what_the_encoding_permits() -> None:
    """Legal and assignable are different questions.

    16-bit addressing reaches memory through bx, bp, si or di; regalloc may
    not hand out bp because it is the frame pointer. Answering both with one
    set made every frame slot look like a violated requirement.
    """
    from qbopt import regalloc

    assert Register.BP in target.ADDRESSING, "a frame slot is reached through bp"
    assert Register.EBP not in target.BASES, "and the allocator may not hand it out"
    assert Register.DX not in target.ADDRESSING, "`[dx+0Ah]` has no encoding"


def test_a_byte_wide_held_is_the_low_byte() -> None:
    """al and ah are both one byte and both root to eax.

    AT_WIDTH was built by assignment, so whichever came last in the byte
    row won -- and that is ah. An ir.Held of width 1 resolved to `ah`,
    which is a different register holding a different byte, and nothing
    would have said so.
    """
    from iced_x86 import Register
    from qbopt import ir
    from qbopt import select

    for root, low in ((Register.EAX, Register.AL), (Register.EBX, Register.BL),
                      (Register.ECX, Register.CL), (Register.EDX, Register.DL)):
        assert select.AT_WIDTH[root][1] is low, f"{root} at one byte is not its low half"
    assert ir.ROOT[Register.AH] is Register.EAX, "the high byte still roots to eax"


def _one_body(stem: str):
    from qbopt import lower

    found = module.of(omf.parse((Path("fixtures/omf") / f"{stem}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    name, body = next(iter(mir.bodies(found, blocks)))
    return lower, name, body, found


def test_lowering_is_one_instruction_per_operation_unless_something_expands() -> None:
    """The default, and it has to stay exactly what it was: every operation
    is one instruction, carrying its own address, span and operation."""
    lower, name, body, found = _one_body("hotlop-p-g2")
    low = lower.lowered(name, body, found.calls)
    ops = [op for block in body.blocks for op in block.ops]
    insns = [one for block in low.blocks for one in block.insns]
    assert len(insns) == len(ops)
    for one, op in zip(insns, ops, strict=True):
        assert (one.at, one.covers, one.op) == (op.at, op.covers, op)


def test_an_expansion_gives_its_leader_the_operation_and_its_followers_none(monkeypatch) -> None:
    """One operation, several instructions. The leader stands for the bytes
    and the rest stand for none: an inserted instruction claiming the same
    span made layout say a byte was held by more than one op.

    What each instruction reads and writes is its own, the leader included.
    A leader computing the expansion's first step while claiming the whole
    operation's operands tells the allocator the multiply's product is live
    from the load -- and the effect the operation had belongs to the run,
    not to any one instruction in it.
    """
    lower, name, body, found = _one_body("hotlop-p-g2")
    kind = next(op.kind for block in body.blocks for op in block.ops if op.kind is mir.Kind.ADD)

    def two(op, making):
        temp = ir.Held(making.fresh(), 4)
        made = next(one for one in op.defines if not one.flags)
        return (
            ir.Semantics(ir.Operation.MOVE, "mov", (temp,), (ir.Imm(1, 4),)),
            ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(made.id, 4),), (temp,)),
        )

    monkeypatch.setitem(lower._EXPANDS, kind, two)
    low = lower.lowered(name, body, found.calls)
    runs = []
    for block in low.blocks:
        for index, one in enumerate(block.insns):
            if one.op is not None and one.op.kind is kind:
                runs.append((one, block.insns[index + 1]))
    assert runs

    for leader, follower in runs:
        original = {one.id for one in leader.op.defines if not one.flags}
        assert follower.covers == (follower.at, follower.at), "a follower stands for no bytes"
        assert follower.op is None and not follower.clobbers
        # Each instruction's own dataflow, from its own semantics.
        assert leader.uses == (), "the leader reads what its first step reads, not the operation's"
        assert set(leader.defines) & original == set(), "the leader makes a temporary, not the result"
        assert set(leader.defines) == set(follower.uses), "the temporary is what joins them"
        assert set(follower.defines) == original
        # And the run as a whole is what the operation was.
        assert original <= set(leader.defines) | set(follower.defines)


def test_an_expansion_invents_values_nothing_else_uses(monkeypatch) -> None:
    """A fresh id per call, and none of them one the body already had."""
    lower, name, body, found = _one_body("hotlop-p-g2")
    had = {
        one.id
        for block in body.blocks
        for op in block.ops
        for one in (*op.defines, *op.uses)
    }
    made = lower.Lowering(body, set(), {}, ())
    got = [made.fresh() for _ in range(8)]
    assert len(set(got)) == len(got)
    assert not (set(got) & had)


def _eliminated(stem: str):
    from qbopt import coalesce
    from qbopt import lower
    from qbopt import phielim
    from qbopt import transform
    from qbopt import twoaddr

    found = module.of(omf.parse((Path("fixtures/omf") / f"{stem}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    name, body = next(iter(mir.bodies(found, blocks)))
    body = transform.widened(
        transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
    )
    low = lower.lowered(name, body, found.calls, set(found.absorbed))
    low = phielim.PhiElimination().transform(low)
    return low, twoaddr.TwoAddress().transform(low), coalesce.Coalescer()


def test_the_moves_one_phi_edge_becomes_are_one_group() -> None:
    """A phi says several values arrive together, so the moves it becomes
    are simultaneous. pressx-v-evt emitted six of them in the order they
    were written and one read a register an earlier one had overwritten --
    `r24 <- [bp-8]` and then `r27 <- r24`, so an arm carried the wrong
    value and R came out 6460 for 7500.

    Which moves belong together cannot be read off their address: every
    one of them sits on the predecessor's last instruction, and so does
    any ordinary move already there.
    """
    low, _after, _c = _eliminated("pressx-v-evt")
    grouped: dict[int, list] = {}
    for block in low.blocks:
        for one in block.insns:
            if one.group is not None:
                grouped.setdefault(one.group, []).append(one)
    assert grouped, "no phi edge was lowered here"
    assert max(len(one) for one in grouped.values()) >= 2, "a group of one proves nothing"
    for number, moves in grouped.items():
        assert len({one.at for one in moves}) == 1, f"group {number} spans addresses"


def test_two_edges_are_two_groups_and_a_plain_move_is_none() -> None:
    """Different edges are different simultaneous sets, and a move that
    was already there belongs to neither."""
    low, _after, _c = _eliminated("pressx-v-evt")
    every = [one for block in low.blocks for one in block.insns]
    groups = {one.group for one in every if one.group is not None}
    assert len(groups) >= 2, f"only {groups} -- this fixture has one edge, so it proves nothing"
    plain = [one for one in every if one.group is None and one.op is not None]
    assert plain, "every instruction was grouped"
    beside = {one.at for one in every if one.group is not None}
    assert any(one.at in beside for one in plain), "no ordinary move shares an address with a group"


def test_the_group_survives_the_phases_after_it() -> None:
    low, after, _c = _eliminated("pressx-v-evt")
    was = {(one.at, one.group) for block in low.blocks for one in block.insns if one.group is not None}
    now = {(one.at, one.group) for block in after.blocks for one in block.insns if one.group is not None}
    assert was and was <= now, "two-address rewriting lost which moves are simultaneous"
