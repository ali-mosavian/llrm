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
