"""
Substituting a memory operand for a register: the invariants.

The transform changes where the second operand is read from and nothing
else. So the test is not "does it encode" -- it is that the mnemonic and
the destination come back identical, because those are exactly what
forward.py got wrong by deleting the instruction instead.
"""

from pathlib import Path

import pytest
from iced_x86 import OpKind
from iced_x86 import Decoder
from iced_x86 import Register

import corpus
from qbopt import mir
from qbopt import omf
from qbopt import avail
from qbopt import memory
from qbopt import reencode
from qbopt.rewrite import plan
from qbopt.declen import BITNESS

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def sites(obj: Path) -> list[tuple]:
    """Every substitution this would make, with the instruction it replaces."""
    found = corpus.loaded(obj)
    assert found is not None
    found_blocks = corpus.partitioned(obj)
    at_of = {insn.at: insn for block in found_blocks for insn in block.insns}
    reported = memory.redundant_loads(found_blocks, found.resolve, found.calls, found.dgroup)
    want = frozenset(at for where in reported.values() for at in where)
    out = []
    for _, body in mir.bodies(found, found_blocks):
        for one in avail.forwardable(body, found.dgroup, found.calls, want):
            insn = at_of.get(one.at)
            emitted = reencode.with_operand(insn, one.root) if insn else None
            if insn is not None and emitted is not None:
                out.append((insn, one.root, emitted))
    return out


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_the_mnemonic_and_destination_survive(obj: Path) -> None:
    """An accumulate keeps accumulating, into the register it always did.

    This is the property forward.py violated. It read `and cx,[x]` as a
    load and deleted it, which is the same forward with the `and` thrown
    away -- arith computed 0f0f0f0f where it wanted 1f3f5f7f on nine of
    twelve real-compiler configurations. Substituting the operand cannot
    lose the operation, and this is what says so.
    """
    for insn, _root, emitted in sites(obj):
        decoded = next(iter(Decoder(BITNESS, emitted.code, ip=insn.at)))
        assert decoded.mnemonic == insn.insn.mnemonic, f"{obj.stem} {insn.at:#x}: mnemonic changed"
        assert decoded.op_count == insn.insn.op_count
        # Every operand except the substituted one comes back untouched.
        # The memory operand is not always op1: `cmp [x],1` has it first,
        # and substituting that is sound because cmp writes neither.
        for index in range(decoded.op_count):
            if insn.insn.op_kind(index) == OpKind.MEMORY:
                continue
            assert decoded.op_kind(index) == insn.insn.op_kind(index), (
                f"{obj.stem} {insn.at:#x}: operand {index} changed kind"
            )
            if insn.insn.op_kind(index) == OpKind.REGISTER:
                assert decoded.op_register(index) == insn.insn.op_register(index), (
                    f"{obj.stem} {insn.at:#x}: operand {index} moved register"
                )


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_the_memory_operand_became_the_provider(obj: Path) -> None:
    """And became that register, not some other one."""
    from qbopt.ir import ROOT

    for insn, root, emitted in sites(obj):
        decoded = next(iter(Decoder(BITNESS, emitted.code, ip=insn.at)))
        replaced = [index for index in range(decoded.op_count) if insn.insn.op_kind(index) == OpKind.MEMORY]
        assert len(replaced) == 1
        became = decoded.op_register(replaced[0])
        assert became != Register.NONE
        assert ROOT.get(became, became) is root, f"{obj.stem} {insn.at:#x}: read from the wrong register"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_no_substitution_reads_memory_any_more(obj: Path) -> None:
    """The whole point: the memory access is gone."""
    for insn, _, emitted in sites(obj):
        decoded = next(iter(Decoder(BITNESS, emitted.code, ip=insn.at)))
        assert all(decoded.op_kind(index) != OpKind.MEMORY for index in range(decoded.op_count))


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_written_memory_operand_is_never_substituted(obj: Path) -> None:
    """`add [x],1` writes its result back to [x]; `add ax,1` does not.

    Substituting a written memory operand deletes the store, silently.
    Two of the twelve real-compiler configurations failed on it, with the
    host suite green -- the same shape of miss as forward.py's accumulate.
    """
    from qbopt.declen import INFO
    from qbopt.declen import WRITES

    for insn, _, _ in sites(obj):
        assert not any(one.access in WRITES for one in INFO.info(insn.insn).used_memory()), (
            f"{obj.stem} {insn.at:#x}: {insn.insn} writes the operand that was substituted"
        )


def test_it_refuses_an_instruction_with_no_memory_operand() -> None:
    for obj in FIXTURES[:4]:
        for block in corpus.partitioned(obj):
            for insn in block.insns:
                if all(insn.insn.op_kind(i) != OpKind.MEMORY for i in range(insn.insn.op_count)):
                    assert reencode.with_operand(insn, Register.EAX) is None
                    return
    raise AssertionError("no register-only instruction in the first four fixtures")


def test_the_emitted_count_is_what_was_measured() -> None:
    """15 emitted at this size, 34 refused to the widening pass, which folds the pair.

    A canary. The refusals are not failures: `and cx,[x]` / `and bx,[x+2]`
    is one 32-bit and, and lift.py claiming it is the better rewrite.
    """
    taken = refused = 0
    for obj in FIXTURES:
        for one in plan(omf.parse(obj.read_bytes())):
            before, after = one.region.before, one.region.after
            if before and len(before) == 8 and (after is None or len(after) == 4):
                if one.region.taken:
                    taken += 1
                elif one.region.reason == "it overlaps a region already taken":
                    refused += 1
    assert (taken, refused) == (15, 34)
