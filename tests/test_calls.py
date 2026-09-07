"""
The runtime calls, and the argument order that is silently a different answer.
"""

from pathlib import Path

import pytest
from iced_x86 import Code
from iced_x86 import OpKind
from iced_x86 import Decoder
from iced_x86 import Mnemonic
from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import Instruction
from iced_x86 import BlockEncoder

import corpus
from helpers import hx
from qbopt import calls
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
from qbopt.calls import consume
from qbopt.calls import grouped
from qbopt.declen import decode
from qbopt.calls import CallSite
from qbopt.calls import MULTIPLY
from qbopt.declen import BITNESS
from qbopt.calls import REMAINDER
from qbopt.calls import LEFT_FIRST
from qbopt.calls import popped_into
from qbopt.calls import FIX_MULTIPLY
from qbopt.calls import fix_multiply


def found_sites(obj: Path) -> dict[str, tuple]:
    parsed = corpus.loaded(obj)
    assert parsed is not None
    reached = corpus.reached(obj)
    assert not isinstance(reached, str)
    return {site.name: site.operands for site in sites(parsed, reached, corpus.partitioned(obj))}


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
    parsed = corpus.loaded(fixtures / "vbdos-g3.obj")
    assert parsed is not None
    reached = corpus.reached(fixtures / "vbdos-g3.obj")
    assert not isinstance(reached, str)
    index = next(i for i, insn in enumerate(reached) if parsed.calls.get(insn.at) == COMPARE)
    assert match(parsed, reached, index) is not None
    # the same call one instruction further back has a push missing
    assert match(parsed, reached[: index - 1] + reached[index:], index - 1) is None


def test_a_pushed_constant_is_not_a_static(fixtures: Path) -> None:
    parsed = corpus.loaded(fixtures / "vbdos-g3.obj")
    assert parsed is not None
    from qbopt.declen import decode
    from qbopt.calls import static_at

    immediate = decode(hx("66 68 78 56 34 12"), 0)  # push dword 0x12345678
    assert immediate is not None
    assert static_at(parsed, immediate) is None


def test_a_pushed_negative_dword_constant_keeps_its_sign() -> None:
    # iced-x86's own immediate() is unsigned for every PUSH*_IMM* form (0x68
    # 78 56 34 12 with no operand-size override, imm32, reads back as
    # 2324436648 -- confirmed by decoding it directly). constant_at() must
    # correct that back to the value BASIC actually pushed, or a call site
    # absorbed against it hands iced-x86's own instruction builder a Python
    # int outside i32 range and it raises OverflowError -- found by
    # tools/fuzzcheck.py generating a LONG multiply against a large negative
    # literal, which crashed qbopt/calls.py's absorb() outright.
    from qbopt.calls import constant_at

    pushed = decode(hx("66 68 a8 16 8c 8a"), 0)  # push dword -1970530648
    assert pushed is not None
    operand = constant_at(pushed)
    assert operand is not None
    assert operand.value == -1970530648


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
    parsed = corpus.loaded(fixtures / "cmpord-v-g3.obj")
    assert parsed is not None
    reached = corpus.reached(fixtures / "cmpord-v-g3.obj")
    assert not isinstance(reached, str)
    found_blocks = corpus.partitioned(fixtures / "cmpord-v-g3.obj")
    site = next(s for s in sites(parsed, reached, found_blocks) if s.name == COMPARE)
    emitted = absorb(site, Flag.NONE)
    assert not isinstance(emitted, str)
    # push eax / mov eax,[a] / cmp eax,[b] / pop eax -- see
    # test_absorbed_compare_restores_eax for why the wrap is there
    assert len(emitted.code) == 13
    assert FIXUP[0] not in emitted.code


def test_absorbed_compare_restores_eax(fixtures: Path) -> None:
    # B$CPI4's real body (runtime/rt/helpi4.asm) never touches cx, dx or bx
    # at all, and its cProc save-list preserves ax too -- a real call to it
    # changes nothing but the flags, so BC's own code can keep a value live
    # in eax right across a compare buried inside a larger expression.
    # Absorbing the call still needs a scratch register to hold one side of
    # the comparison, but it has to come back exactly as it was found.
    parsed = corpus.loaded(fixtures / "cmpord-v-g3.obj")
    assert parsed is not None
    reached = corpus.reached(fixtures / "cmpord-v-g3.obj")
    assert not isinstance(reached, str)
    found_blocks = corpus.partitioned(fixtures / "cmpord-v-g3.obj")
    site = next(s for s in sites(parsed, reached, found_blocks) if s.name == COMPARE)
    emitted = absorb(site, Flag.NONE)
    assert not isinstance(emitted, str)
    decoded = list(Decoder(BITNESS, emitted.code, ip=0))
    assert decoded[0].mnemonic == Mnemonic.PUSH and decoded[0].op0_register == Register.EAX
    assert decoded[-1].mnemonic == Mnemonic.POP and decoded[-1].op0_register == Register.EAX
    # nothing between the push and the pop writes back to eax -- the load
    # and the cmp both read/write it, which is exactly what gets undone
    inner = decoded[1:-1]
    assert any(insn.op0_register == Register.EAX for insn in inner)


def test_absorbed_compare_against_a_constant_wraps_eax_too() -> None:
    # apply_to's compact eax-specific immediate form (CMP_EAX_IMM32) is still
    # the one used here -- eax is not off limits, only unrestored, so nothing
    # stops the short form the way a genuinely different register would.
    site = CallSite(at=0, end=0, start=0, name=COMPARE, pushed=(static_operand(0x10), constant_operand(70000)))
    emitted = absorb(site, Flag.NONE)
    assert not isinstance(emitted, str)
    decoded = list(Decoder(BITNESS, emitted.code, ip=0))
    assert decoded[0].mnemonic == Mnemonic.PUSH and decoded[0].op0_register == Register.EAX
    assert decoded[-1].mnemonic == Mnemonic.POP and decoded[-1].op0_register == Register.EAX
    assert Mnemonic.CMP in {insn.mnemonic for insn in decoded}


# the opcode's third byte is the only difference between the shrd forms:
# AC takes an imm8 count, AD takes cl
SHRD_IMM8 = bytes.fromhex("660fac")
SHRD_CL = bytes.fromhex("660fad")


def static_operand(offset: int, base: Register_ = Register.NONE) -> Operand:
    return Operand(Kind.STATIC, module.Addr(module.Space.SEGMENT, offset, base=base), at=offset, length=1)


def constant_operand(value: int) -> Operand:
    return Operand(Kind.CONSTANT, value=value, length=1)


def test_squaring_the_same_address_loads_it_once() -> None:
    x = static_operand(0x76)
    site = CallSite(at=0, end=0, start=0, name=MULTIPLY, pushed=(x, x))
    emitted = absorb(site, Flag.NONE)
    assert not isinstance(emitted, str)
    # mov eax,ds:[x] / imul eax,eax / push eax,pop ax,pop dx -- one load, not two
    assert emitted.code == hx("66 A1 00 00  66 0F AF C0  66 50 58 5A")
    assert emitted.relocations == ((2, 0x76),), "one fixup, not two, for the one address read"


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


def test_grouped_splits_mixed_shapes_by_byte_count() -> None:
    # a word pair (deepest) then a dword push (topmost) -- stack.py guarantees
    # the total is a multiple of four, so byte-counting from the top always
    # lands on the boundary between two arguments regardless of their shapes.
    hi, lo = decode(hx("52"), 0), decode(hx("50"), 0)  # push dx / push ax
    dword = decode(hx("66 FF 36 00 00"), 0)  # push dword [x]
    assert hi is not None and lo is not None and dword is not None
    assert grouped((hi, lo, dword)) == [(hi, lo), (dword,)]


def test_grouped_refuses_a_word_pair_split_across_two_arguments() -> None:
    # stack.py guarantees the TOTAL is a multiple of four, not that a word
    # pair stays adjacent to its own other half rather than a neighbour's --
    # this shape (word, dword, word) sums to 8 but no 4-byte prefix from the
    # top is a real argument, and grouped() must refuse rather than hand
    # consume() a group that isn't actually one push's own pair.
    w1, w2 = decode(hx("52"), 0), decode(hx("50"), 0)  # push dx / push ax
    dword = decode(hx("66 FF 36 00 00"), 0)  # push dword [x]
    assert w1 is not None and w2 is not None and dword is not None
    assert grouped((w1, dword, w2)) is None


def test_consume_refuses_rather_than_crashes_on_an_ungroupable_frame() -> None:
    w1, w2 = decode(hx("52"), 0), decode(hx("50"), 0)
    dword = decode(hx("66 FF 36 00 00"), 0)
    assert w1 is not None and w2 is not None and dword is not None
    site = CallSite(at=0, end=0, start=0, name=MULTIPLY, consume=(w1, dword, w2))
    assert isinstance(consume(site, Flag.NONE), str)


def test_popped_into_is_a_bare_pop() -> None:
    assert popped_into(Register.ECX) == Instruction.create_reg(Code.POP_R32, Register.ECX)


def test_consume_pops_a_dword_and_a_word_pair_for_compare() -> None:
    # bp cannot hold anything but the frame it builds here, and sp itself
    # cannot be a 16-bit addressing base at all -- see compare_consume()'s
    # own docstring for why the popped arguments are read in place through
    # bp instead of popped into a register the way every other consumed
    # call's arguments are.
    hi, lo = decode(hx("52"), 0), decode(hx("50"), 0)  # push dx / push ax
    dword = decode(hx("66 FF 36 00 00"), 0)  # push dword [x]
    assert hi is not None and lo is not None and dword is not None
    site = CallSite(at=0, end=0, start=0, name=COMPARE, consume=(hi, lo, dword))
    emitted = consume(site, Flag.NONE)
    assert not isinstance(emitted, str)
    assert emitted.relocations == (), "nothing here is a relocated address"
    decoded = list(Decoder(BITNESS, emitted.code, ip=0))
    assert decoded[0].mnemonic == Mnemonic.PUSH and decoded[0].op0_register == Register.BP
    assert decoded[1].mnemonic == Mnemonic.PUSH and decoded[1].op0_register == Register.EDX
    assert Mnemonic.CMP in {insn.mnemonic for insn in decoded}
    # bp and edx both come back, and sp ends up exactly 8 bytes shallower --
    # both original 4-byte arguments consumed. Restoring bp is a pop, not a
    # bp-relative read, because that slot has to be read at or above sp (see
    # test_consume_compare_never_reads_below_the_stack_pointer); the earlier
    # bp-relative lea only gets sp to where the parked value sits, one pop
    # short of the final +8 -- iced's stack_pointer_increment does not
    # follow an arbitrary write to sp, so the two pushes (-6), this lea's
    # own displacement (+12) and the final pop (+2) are the whole story.
    lea = next(insn for insn in decoded if insn.mnemonic == Mnemonic.LEA)
    assert lea.op0_register == Register.SP
    assert lea.memory_displacement == 12
    assert sum(insn.stack_pointer_increment for insn in decoded) + 12 == 8
    assert decoded[-1].mnemonic == Mnemonic.POP and decoded[-1].op0_register == Register.BP


def simulated_reads_never_land_below_sp(code: bytes, entry_sp: int = 0x2000) -> None:
    """Walk a 16-bit instruction stream, tracking sp/bp and every general
    register push/pop/mov touches, and fail the moment something reads
    memory at an address below the *current* sp.

    That memory is not somehow free to use just because nothing here still
    names it -- DOS services interrupts at instruction boundaries, and every
    one of them pushes onto whatever stack is live, at addresses at and
    below sp. A read below sp is reading memory an interrupt handler is free
    to have already clobbered, whether or not anything happens to be
    listening on this particular run.
    """
    regs: dict[Register_, int] = {Register.BP: 0x4000, Register.EDX: 0x1234_5678, Register.SP: entry_sp}
    mem: dict[int, int] = {}

    def widths(reg: Register_) -> int:
        return 4 if reg in (Register.EAX, Register.ECX, Register.EDX, Register.EBX) else 2

    def mem_addr(insn: Instruction) -> int:
        base = insn.memory_base
        assert base in regs, f"unhandled base register {base}"
        return regs[base] + insn.memory_displacement

    def read_mem(addr: int, size: int) -> int:
        assert addr >= regs[Register.SP], f"read at {addr:#x} is below sp {regs[Register.SP]:#x}"
        return mem.get(addr, 0) & ((1 << (size * 8)) - 1)

    def write_mem(addr: int, value: int, size: int) -> None:
        mem[addr] = value & ((1 << (size * 8)) - 1)

    for insn in Decoder(BITNESS, code, ip=0):
        match insn.mnemonic:
            case Mnemonic.PUSH:
                size = widths(insn.op0_register)
                regs[Register.SP] -= size
                write_mem(regs[Register.SP], regs[insn.op0_register], size)
            case Mnemonic.POP:
                size = widths(insn.op0_register)
                regs[insn.op0_register] = read_mem(regs[Register.SP], size)
                regs[Register.SP] += size
            case Mnemonic.MOV if insn.op1_kind == OpKind.MEMORY:
                regs[insn.op0_register] = read_mem(mem_addr(insn), widths(insn.op0_register))
            case Mnemonic.MOV if insn.op0_kind == OpKind.MEMORY:
                write_mem(mem_addr(insn), regs[insn.op1_register], widths(insn.op1_register))
            case Mnemonic.MOV:
                regs[insn.op0_register] = regs[insn.op1_register]
            case Mnemonic.CMP:
                read_mem(mem_addr(insn), widths(insn.op0_register))  # value unused, only the safety check matters
            case Mnemonic.LEA:
                regs[insn.op0_register] = mem_addr(insn)
            case other:
                raise AssertionError(f"simulator does not know {other}")


def test_consume_compare_never_reads_below_the_stack_pointer() -> None:
    # A DOS interrupt handler is free to use everything at and below sp at
    # any instruction boundary -- compare_consume() borrows bp as a frame
    # pointer inside the call's own dead argument space, and every register
    # it restores from there has to be read at or above sp, never below it,
    # or an interrupt landing between two of its instructions can corrupt
    # bp or edx before they come back.
    hi, lo = decode(hx("52"), 0), decode(hx("50"), 0)  # push dx / push ax
    dword = decode(hx("66 FF 36 00 00"), 0)  # push dword [x]
    assert hi is not None and lo is not None and dword is not None
    site = CallSite(at=0, end=0, start=0, name=COMPARE, consume=(hi, lo, dword))
    emitted = consume(site, Flag.NONE)
    assert not isinstance(emitted, str)
    simulated_reads_never_land_below_sp(emitted.code)


def test_consume_pops_every_argument_even_one_shaped_like_a_static() -> None:
    # A push that would classify as Kind.STATIC if match() had found it is
    # still popped here, never reloaded from its address -- match() already
    # owns every site where reloading is sound, and this is only ever asked
    # about a site it refused. Reloading one push while leaving it on the
    # stack would leak four bytes per call, forever.
    static_looking = decode(hx("66 FF 36 00 00"), 0)  # push dword [x]
    hi, lo = decode(hx("52"), 0), decode(hx("50"), 0)  # push dx / push ax
    assert static_looking is not None and hi is not None and lo is not None
    site = CallSite(at=0, end=0, start=0, name=MULTIPLY, consume=(static_looking, hi, lo))
    emitted = consume(site, Flag.NONE)
    assert not isinstance(emitted, str)
    assert emitted.relocations == (), "nothing here is a relocated address to reuse a fixup for"
    decoded = list(Decoder(16, emitted.code, ip=0))
    assert not any(insn.is_ip_rel_memory_operand or insn.memory_base != Register.NONE for insn in decoded), (
        "nothing reads the static-looking push's address -- it was popped, not reloaded"
    )
    assert Mnemonic.POP in {insn.mnemonic for insn in decoded}
    # every argument byte popped and nothing else left on the stack: a build
    # that only popped the stack-only operand and reloaded the static-looking
    # one instead would net +4 here, not +8, and leak the other four forever
    assert sum(insn.stack_pointer_increment for insn in decoded) == 8


def test_consume_divide_puts_the_dividend_in_eax_not_the_divisor() -> None:
    # DIVIDE pushes the divisor first/deepest and the dividend second/topmost
    # (LEFT_FIRST is False for it) -- swapping which one lands in eax is a
    # silent wrong answer, not a crash, so this is pinned byte-exact rather
    # than only checked for length or mnemonic.
    divisor = decode(hx("66 FF 36 00 00"), 0)
    dividend = decode(hx("66 FF 76 00 00"), 0)
    assert divisor is not None and dividend is not None
    site = CallSite(at=0, end=0, start=0, name=DIVIDE, consume=(divisor, dividend))
    emitted = consume(site, Flag.NONE)
    assert not isinstance(emitted, str)
    # pop eax (dividend) / pop ecx (divisor) / cdq / idiv ecx / restore
    assert emitted.code == hx("66 58  66 59  66 99  66 F7 F9  66 50 58 5A")


def test_absorb_reloads_a_classified_frame_site_rather_than_popping_it(fixtures: Path) -> None:
    # lngmix re-pushes v and 7 for its second divide (B$RMI4), and BC puts a
    # store between that push run and the call -- match() cannot reach it, so
    # frames() finds it and leaves it with `consume` set. Its pushes are
    # still classifiable, address and constant like any other, and `absorb`
    # dispatching on `site.consume` alone sent it to `consume()` regardless:
    # the real pushes are never re-emitted (the raise skips them, the same
    # as any folded site), so `consume()`'s pop read garbage off the stack
    # instead of the value BC pushed there: lngmix-p-g2 stopped early under
    # DOSBox (e2e reported NODONE) before this was found.
    parsed = corpus.loaded(fixtures / "lngmix-p-g2.obj")
    assert parsed is not None
    reached = corpus.reached(fixtures / "lngmix-p-g2.obj")
    assert not isinstance(reached, str)
    found = [
        one for one in sites(parsed, reached, corpus.partitioned(fixtures / "lngmix-p-g2.obj")) if one.name == REMAINDER
    ]
    assert len(found) == 1, f"expected one B$RMI4 site, found {len(found)}"
    site = found[0]
    assert site.consume and site.pushed, "the shape this bug needs: both a frame and a classification"
    emitted = absorb(site, Flag.NONE)
    assert not isinstance(emitted, str), emitted
    first = next(iter(Decoder(16, emitted.code, ip=0)))
    assert first.mnemonic == Mnemonic.MOV, (
        f"first instruction was {first.mnemonic!r}, not a load -- the operand was popped, not reloaded"
    )


def test_fix_multiply_consume_uses_edx_and_the_variable_shift_form() -> None:
    # All three arguments stack-only: shift can never use shrd's immediate
    # form here (that needs the value at codegen time, which a popped operand
    # never has), and b has nowhere to go but edx once a and shift take
    # eax and ecx -- imul's one-operand form reads edx before it writes
    # edx:eax, so b survives exactly long enough to be multiplied. Pushes are
    # identical bytes on purpose: only the pop ORDER (a deepest, pushed in
    # source order first; shift topmost) can distinguish a correct target
    # mapping from a role swap, since nothing in a bare `pop eax` names which
    # argument it came from.
    a, b, shift = (decode(hx("66 FF 36 00 00"), 0) for _ in range(3))
    assert a is not None and b is not None and shift is not None
    site = CallSite(at=0, end=0, start=0, name=FIX_MULTIPLY, consume=(a, b, shift))
    emitted = consume(site, Flag.NONE)
    assert not isinstance(emitted, str)
    # pop ecx (shift) / pop edx (b) / pop eax (a) / imul edx / shrd eax,edx,cl / restore
    assert emitted.code == hx("66 59  66 5A  66 58  66 F7 EA  66 0F AD D0  66 50 58 5A")
    assert SHRD_CL in emitted.code
    assert SHRD_IMM8 not in emitted.code
    assert bytes([0x66, 0xF7, 0xEA]) in emitted.code, "imul edx, the one-operand form"


def test_consume_refuses_a_compare_whose_synthesised_flags_are_read() -> None:
    dword = decode(hx("66 FF 36 00 00"), 0)
    assert dword is not None
    site = CallSite(at=0, end=0, start=0, name=COMPARE, consume=(dword, dword))
    assert isinstance(consume(site, Flag.CF), str)


def test_consume_refuses_a_call_whose_arity_does_not_match_what_was_pushed() -> None:
    dword = decode(hx("66 FF 36 00 00"), 0)
    assert dword is not None
    site = CallSite(at=0, end=0, start=0, name=MULTIPLY, consume=(dword,))
    assert isinstance(consume(site, Flag.NONE), str)


@pytest.mark.parametrize(
    ("value", "expect"),
    [
        (2, "shl eax,1"),
        (4, "shl eax,2"),
        (16, "shl eax,4"),
        (256, "shl eax,8"),
        (3, "lea eax,[eax+eax*2]"),
        (5, "lea eax,[eax+eax*4]"),
        (9, "lea eax,[eax+eax*8]"),
    ],
)
def test_a_constant_multiply_the_386_can_do_without_multiplying(value: int, expect: str) -> None:
    """gcc and clang at -O3 pick these on i386, and ×3 is one lea rather
    than a shift and an add. Safe only because absorb() has already refused
    any MULTIPLY site whose flags are read afterwards: imul writes them,
    shl writes them differently, and lea writes none at all.
    """
    found = calls._without_multiplying(value)
    assert found is not None
    encoder = BlockEncoder(BITNESS)
    encoder.add(found)
    assert str(next(iter(Decoder(BITNESS, encoder.encode(0), ip=0)))) == expect


@pytest.mark.parametrize("value", [0, 1, -1, -4, 7, 10, 100])
def test_anything_else_still_multiplies(value: int) -> None:
    """Refused rather than special-cased. ×7 is a lea and a sub and ×-4 a
    shift and a negation -- two instructions and a different shape, which
    nothing in the corpus asks for, and inventing a sequence unmeasured is
    how a fold acquires a case nothing checks."""
    assert calls._without_multiplying(value) is None


def test_strength_reduction_only_applies_to_the_multiply() -> None:
    """B$CPI4's own absorbed form is a compare, and a compare by 3 is not a
    lea by any reading."""
    operand = calls.Operand(calls.Kind.CONSTANT, value=3)
    encoder = BlockEncoder(BITNESS)
    encoder.add(calls.apply_to(calls.COMPARE, operand))
    assert "cmp" in str(next(iter(Decoder(BITNESS, encoder.encode(0), ip=0))))


@pytest.mark.parametrize("value", [2, 16, 512, 262144, 1 << 31])
def test_a_power_of_two_divisor_is_recognised(value: int) -> None:
    assert calls._power_of_two(value) == value.bit_length() - 1


@pytest.mark.parametrize("value", [0, 1, -2, -512, 3, 7, 100, (1 << 32)])
def test_anything_else_is_not(value: int) -> None:
    """1 is excluded with the rest: dividing by it is a no-op, not a shift by
    zero, and a sequence for a case nothing asks for is a case nothing checks."""
    assert calls._power_of_two(value) is None


MASK = 0xFFFFFFFF


def _signed(v: int) -> int:
    return v - (1 << 32) if v & 0x80000000 else v


def _model(name: str, x: int, n: int) -> int:
    """The sequence dividing_by_a_power_of_two emits, executed."""
    eax, scratch = x & MASK, x & MASK
    scratch = (_signed(scratch) >> 31) & MASK
    scratch = (scratch & MASK) >> (32 - n)
    if name == calls.REMAINDER:
        scratch = (scratch + eax) & MASK
        scratch &= -(1 << n) & MASK
        return _signed((eax - scratch) & MASK)
    eax = (eax + scratch) & MASK
    return _signed((_signed(eax) >> n) & MASK)


def _truncating(a: int, b: int) -> int:
    q = abs(a) // abs(b)
    return q if (a < 0) == (b < 0) else -q


@pytest.mark.parametrize("n", range(1, 32))
@pytest.mark.parametrize(
    "x", [0, 1, -1, 2, -2, 511, 512, 513, -511, -512, -513, 2**31 - 1, -(2**31), 123456789, -123456789]
)
def test_the_shift_sequence_truncates_towards_zero_like_idiv(n: int, x: int) -> None:
    """A signed shift alone rounds towards minus infinity -- -1 >> 1 is -1,
    not 0 -- and the language and idiv both truncate towards zero. The bias
    is the whole difference, and getting it wrong is off-by-one on every
    negative dividend, which no byte comparison would catch.
    """
    divisor = 1 << n
    assert _model(calls.DIVIDE, x, n) == _truncating(x, divisor)
    assert _model(calls.REMAINDER, x, n) == x - _truncating(x, divisor) * divisor
