"""
The runtime calls, and the argument order that is silently a different answer.
"""

from pathlib import Path

import pytest

from helpers import hx
from qbopt import module
from qbopt.flags import Flag
from qbopt.lift import FIXUP
from qbopt.calls import match
from qbopt.calls import sites
from qbopt.calls import DIVIDE
from qbopt.calls import absorb
from qbopt.calls import COMPARE
from qbopt.calls import LEFT_FIRST
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
def test_only_comparison_pushes_its_left_operand_first(name: str, left_first: bool) -> None:
    assert left_first == (name == COMPARE)


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
