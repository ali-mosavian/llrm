"""
Naming a call's arguments by stack position, not by adjacency to the call.
"""

from qbopt.frontend.declen import run
from qbopt.frontend.blocks import Ends
from qbopt.frontend.blocks import Block
from qbopt.frontend.stack import frames

ARITY = {"F1": 1, "F2": 2}


def block_of(hexcode: str) -> Block:
    code = bytes.fromhex(hexcode.replace(" ", ""))
    insns, stuck = run(code, 0, len(code))
    assert stuck is None, "the test's own bytes must decode cleanly"
    return Block(at=0, end=len(code), insns=tuple(insns), ends=Ends.FALLS_THROUGH, succ=())


def arity(name: str | None) -> int | None:
    return ARITY.get(name) if name is not None else None


def test_two_contiguous_pushes_form_one_frame() -> None:
    block = block_of("66 FF 36 00 00  66 FF 36 00 00  9A 00 00 00 00")
    call = block.insns[-1]
    found = frames(block, {call.at: "F2"}, arity)
    assert len(found) == 1
    assert found[0].call is call
    assert found[0].pushed == block.insns[:2]


def test_a_value_pushed_early_is_found_under_a_nested_call() -> None:
    # push A -- for the OUTER call, but not consumed until the very end
    # push B / push C / call INNER(2) -- entirely self-contained, consumes B and C
    # push D -- the outer call's second argument
    # call OUTER(2) -- consumes A (stranded beneath the inner call) and D
    block = block_of(
        "66 FF 36 00 00"  # push A
        "66 FF 36 00 00"  # push B
        "66 FF 36 00 00"  # push C
        "9A 00 00 00 00"  # call INNER
        "66 FF 36 00 00"  # push D
        "9A 00 00 00 00"  # call OUTER
    )
    a, b, c, inner, d, outer = block.insns
    found = frames(block, {inner.at: "F2", outer.at: "F2"}, arity)
    assert len(found) == 2
    assert found[0].call is inner
    assert found[0].pushed == (b, c)
    assert found[1].call is outer
    assert found[1].pushed == (a, d), "a survives the nested call's own push and pop"


def test_an_ordinary_instruction_between_pushes_is_not_a_gap() -> None:
    # mov ax,bp between two pushes touches neither sp nor either pushed value
    # -- an outer call spanning it should see both, exactly like the nested
    # call case above but through a plain instruction instead of a call.
    block = block_of(
        "66 FF 36 00 00"  # push A
        "8B C5"  # mov ax,bp -- provably does not touch sp
        "66 FF 36 00 00"  # push B
        "9A 00 00 00 00"  # call F2
    )
    a, _mov, b, call = block.insns
    found = frames(block, {call.at: "F2"}, arity)
    assert len(found) == 1
    assert found[0].pushed == (a, b)


def test_a_pop_is_a_gap() -> None:
    block = block_of("66 FF 36 00 00  58  66 FF 36 00 00  9A 00 00 00 00")
    call = block.insns[-1]
    assert frames(block, {call.at: "F2"}, arity) == []


def test_arithmetic_on_sp_is_a_gap_even_though_iced_reports_no_increment() -> None:
    # add sp,8 -- iced's own stack_pointer_increment is 0 for this (it is not
    # a "stack instruction" to iced), so only the register-write check catches
    # it; missing that would let A silently look adjacent to the call.
    block = block_of("66 FF 36 00 00  83 C4 08  66 FF 36 00 00  9A 00 00 00 00")
    call = block.insns[-1]
    assert frames(block, {call.at: "F2"}, arity) == []


def test_a_call_needing_more_than_was_pushed_is_not_a_frame() -> None:
    block = block_of("66 FF 36 00 00  9A 00 00 00 00")  # one push, a call needing two
    call = block.insns[-1]
    assert frames(block, {call.at: "F2"}, arity) == []


def test_a_straddled_push_refuses_rather_than_over_counts() -> None:
    # push dword(4) then push ax(2) -- 6 bytes on the stack, and F1 needs 4:
    # the top push alone is not enough and the dword below is not all of it.
    block = block_of("66 FF 36 00 00  50  9A 00 00 00 00")
    call = block.insns[-1]
    assert frames(block, {call.at: "F1"}, arity) == []


def test_refusal_reset_must_not_be_reachable_past() -> None:
    # push ax(2) / call F1 (needs 4, refused, resets) / push ax(2) / call F1
    # -- the two 2-byte pushes must not be added across the refused call.
    block = block_of("50  9A 00 00 00 00  50  9A 00 00 00 00")
    first_call, second_call = block.insns[1], block.insns[3]
    found = frames(block, {first_call.at: "F1", second_call.at: "F1"}, arity)
    assert found == []


def test_an_unrecognised_call_mid_block_is_a_gap() -> None:
    block = block_of("66 FF 36 00 00  9A 00 00 00 00  66 FF 36 00 00  9A 00 00 00 00")
    first_call, second_call = block.insns[1], block.insns[3]
    found = frames(block, {first_call.at: "NOPE", second_call.at: "F1"}, arity)
    assert len(found) == 1
    assert found[0].call is second_call
    assert found[0].pushed == (block.insns[2],)


def test_mixed_arity_in_one_block() -> None:
    # F1(1) then F2(2), back to back, each popping a different amount.
    block = block_of(
        "66 FF 36 00 00"  # push A
        "9A 00 00 00 00"  # call F1 -- consumes A alone
        "66 FF 36 00 00"  # push B
        "66 FF 36 00 00"  # push C
        "9A 00 00 00 00"  # call F2 -- consumes B and C
    )
    a, f1, b, c, f2 = block.insns
    found = frames(block, {f1.at: "F1", f2.at: "F2"}, arity)
    assert len(found) == 2
    assert found[0].pushed == (a,)
    assert found[1].pushed == (b, c)


def test_arity_under_one_is_not_a_nameable_call() -> None:
    block = block_of("66 FF 36 00 00  9A 00 00 00 00")
    call = block.insns[-1]
    assert frames(block, {call.at: "F0"}, lambda name: {"F0": 0}.get(name)) == []
