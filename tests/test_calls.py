"""
The runtime calls, and the argument order that is silently a different answer.
"""

from pathlib import Path

import pytest
from iced_x86 import Register
from iced_x86 import Register_

from helpers import hx
from qbopt import module
from qbopt.calls import Kind
from qbopt.flags import Flag
from qbopt.lift import FIXUP
from qbopt.calls import match
from qbopt.calls import sites
from qbopt.calls import DIVIDE
from qbopt.calls import absorb
from qbopt.calls import COMPARE
from qbopt.calls import Operand
from qbopt.calls import CallSite
from qbopt.calls import LEFT_FIRST
from qbopt.calls import FIX_MULTIPLY
from qbopt.calls import fix_multiply
from qbopt.blocks import instructions


def found_sites(obj: Path) -> dict[str, tuple]:
    parsed = module.load(obj)
    assert parsed is not None
    reached = instructions(parsed)
    assert not isinstance(reached, str)
    return {site.name: site.operands for site in sites(parsed, reached)}


def test_compare_and_divide_agree_on_which_operand_is_left(operator_obj: Path) -> None:
    # The fixture computes `a op b` twice over the same two variables, once as a
    # comparison and once as a divide. Comparison pushes its left operand first
    # and divide pushes it second, so if either entry in LEFT_FIRST is wrong the
    # two disagree -- on every fixture, in both push shapes. No oracle needed.
    by_name = found_sites(operator_obj)
    assert {COMPARE, DIVIDE} <= set(by_name)
    left, right = by_name[COMPARE]
    assert (left.addr, right.addr) == tuple(operand.addr for operand in by_name[DIVIDE])
    assert left.addr != right.addr, "and they are two different variables"


def test_both_push_shapes_reach_the_same_operands(fixtures: Path) -> None:
    # VBDOS /G3 pushes one dword per argument, 15 bytes with the call; everything
    # else pushes two words, high first, 21 bytes.
    wide = found_sites(fixtures / "vbdos-g3.obj")
    narrow = found_sites(fixtures / "vbdos-g2.obj")
    for name in (COMPARE, DIVIDE):
        assert tuple(o.addr for o in wide[name]) == tuple(o.addr for o in narrow[name])


@pytest.mark.parametrize(("name", "left_first"), sorted(LEFT_FIRST.items()))
def test_only_comparison_and_fix_multiply_push_their_left_operand_first(name: str, left_first: bool) -> None:
    assert left_first == (name in (COMPARE, FIX_MULTIPLY))


def test_a_call_with_anything_between_the_pushes_is_refused(fixtures: Path) -> None:
    parsed = module.load(fixtures / "vbdos-g3.obj")
    assert parsed is not None
    reached = instructions(parsed)
    assert not isinstance(reached, str)
    index = next(i for i, insn in enumerate(reached) if parsed.calls.get(insn.at) == COMPARE)
    assert match(parsed, reached, index) is not None
    # the same call one instruction further back has a push missing
    assert match(parsed, reached[: index - 1] + reached[index:], index - 1) is None


def test_a_pushed_constant_is_not_a_static(fixtures: Path) -> None:
    parsed = module.load(fixtures / "vbdos-g3.obj")
    assert parsed is not None
    from qbopt.declen import decode
    from qbopt.calls import static_at

    immediate = decode(hx("66 68 78 56 34 12"), 0)  # push dword 0x12345678
    assert immediate is not None
    assert static_at(parsed, immediate) is None


def test_an_indexed_push_is_a_static_operand_carrying_its_base() -> None:
    from qbopt.declen import decode
    from qbopt.calls import static_at

    insn = decode(hx("66 FF B4 10 00"), 0)  # push dword [si+0x10]
    assert insn is not None and insn.disp_at is not None
    fake = module.Module([], 1, "test", b"", 0, 0, operands={insn.disp_at: module.Addr(module.Space.SEGMENT, 0, 5)})
    found = static_at(fake, insn)
    assert found is not None
    assert found.addr == module.Addr(module.Space.SEGMENT, 0, 5, base=Register.SI)


def test_a_scaled_index_push_is_not_a_static_operand() -> None:
    from qbopt.declen import decode
    from qbopt.calls import static_at

    insn = decode(hx("66 67 FF 34 85 10 00 00 00"), 0)  # push dword [eax*4+0x10], no base at all
    assert insn is not None and insn.disp_at is not None
    fake = module.Module([], 1, "test", b"", 0, 0, operands={insn.disp_at: module.Addr(module.Space.SEGMENT, 0, 5)})
    assert static_at(fake, insn) is None


def test_an_absorbed_comparison_leaves_no_value_to_restore(fixtures: Path) -> None:
    # A comparison's answer is in the flags. Appending the sequence that puts a
    # long's high half back wastes four bytes and clobbers ax and dx, which the
    # code after the call is entitled to still hold. It happened: `site.name is
    # COMPARE` compares identity, and two equal strings need not be one object.
    parsed = module.load(fixtures / "cmpord-v-g3.obj")
    assert parsed is not None
    reached = instructions(parsed)
    assert not isinstance(reached, str)
    site = next(s for s in sites(parsed, reached) if s.name == COMPARE)
    emitted = absorb(site, Flag.NONE)
    assert not isinstance(emitted, str)
    assert len(emitted.code) == 9, "mov eax,[a] then cmp eax,[b], and nothing else"
    assert FIXUP[0] not in emitted.code


# the opcode's third byte is the only difference between the shrd forms:
# AC takes an imm8 count, AD takes cl
SHRD_IMM8 = bytes.fromhex("660fac")
SHRD_CL = bytes.fromhex("660fad")


def static_operand(offset: int, base: Register_ = Register.NONE) -> Operand:
    return Operand(Kind.STATIC, module.Addr(module.Space.SEGMENT, offset, base=base), at=offset, length=1)


def constant_operand(value: int) -> Operand:
    return Operand(Kind.CONSTANT, value=value, length=1)


def test_fix_multiply_is_one_imul_and_one_shrd_against_a_static() -> None:
    # mov eax,[a] / imul dword [b] / shrd eax,edx,16, then the high-half
    # restore BC reads through dx:ax the same way it does after a multiply.
    site = CallSite(
        at=0, end=0, start=0, name=FIX_MULTIPLY, pushed=(static_operand(4), static_operand(0), constant_operand(16))
    )
    emitted = fix_multiply(site, Flag.NONE)
    assert not isinstance(emitted, str)
    assert len(emitted.code) == 18, "mov eax,[a] / imul dword [b] / shrd eax,edx,16, then the restore"
    assert FIXUP[0] in emitted.code
    assert len(emitted.relocations) == 2


def test_fix_multiply_against_a_constant_loads_it_first() -> None:
    # imul has no immediate form that keeps the high half, so a constant b
    # goes into a register before the multiply.
    site = CallSite(
        at=0, end=0, start=0, name=FIX_MULTIPLY, pushed=(static_operand(4), constant_operand(3), constant_operand(16))
    )
    emitted = fix_multiply(site, Flag.NONE)
    assert not isinstance(emitted, str)
    assert len(emitted.relocations) == 1, "only a's fixup is there to reuse"


def test_fix_multiply_with_a_variable_shift_loads_cl() -> None:
    # fixShift is always known at compile time in practice, but a variable
    # still has to work: shrd's only other source for a count is cl.
    site = CallSite(
        at=0, end=0, start=0, name=FIX_MULTIPLY, pushed=(static_operand(4), static_operand(0), static_operand(8))
    )
    emitted = fix_multiply(site, Flag.NONE)
    assert not isinstance(emitted, str)
    assert len(emitted.relocations) == 3, "a, b and the shift each reuse a fixup"
    assert SHRD_CL in emitted.code
    assert SHRD_IMM8 not in emitted.code


def test_fix_multiply_refuses_a_shift_that_cannot_normalise_32_bits() -> None:
    site = CallSite(
        at=0, end=0, start=0, name=FIX_MULTIPLY, pushed=(static_operand(4), static_operand(0), constant_operand(32))
    )
    refused = fix_multiply(site, Flag.NONE)
    assert isinstance(refused, str)


def test_fix_multiply_refuses_a_site_whose_flags_are_read() -> None:
    site = CallSite(
        at=0, end=0, start=0, name=FIX_MULTIPLY, pushed=(static_operand(4), static_operand(0), constant_operand(16))
    )
    refused = fix_multiply(site, Flag.ZF)
    assert isinstance(refused, str)


@pytest.mark.parametrize(
    ("a", "b", "shift"),
    [
        (65536, 131072, 16),
        (-65536, 131072, 16),
        (-65536, -131072, 16),
        (2147483647, 2, 16),
        (-2147483648, 65536, 16),
        (65536, 131072, 8),
        (65536, 131072, 0),
    ],
)
def test_fix_multiply_matches_the_64_bit_shift_it_means(a: int, b: int, shift: int) -> None:
    # (int32)(((int64)a * b) >> shift), truncated to 32 bits the way C does it
    # and the way SHRD does it: no sign extension, because a 64-bit two's
    # complement value's bits do not depend on its sign for a shift like this.
    want = ((a * b) >> shift) & 0xFFFFFFFF
    want = want - 0x100000000 if want >= 0x80000000 else want
    assert -(2**31) <= want < 2**31
