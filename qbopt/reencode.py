"""
The same instruction, in a different register.

regalloc.py decides where a value should live; nothing could act on that
because every rewrite so far either replaced a whole region or deleted an
instruction outright. Moving a value means emitting the instruction BC
wrote with one of its registers changed, which is a smaller thing than
instruction selection and the piece that was missing.

Two things make it more than setting a field.

**Not every encoding takes every register.** `mov eax,[1234h]` is the moffs
form, two bytes shorter than the general one and available to the
accumulator alone; ask iced to put ecx in it and it refuses rather than
silently emitting something else. The same is true of the accumulator forms
of add, sub, cmp and the rest. Where the form cannot hold the register the
answer here is None -- refusing, not reaching for a different encoding,
because a different encoding is a different length and that is the caller's
problem to know about rather than this function's to hide.

**A relocated displacement moves.** An operand's fixup is recorded against
a byte offset in the original instruction, and re-encoding can put the
displacement somewhere else. The new offset is read back off the encoded
bytes rather than predicted, which is what calls.py's own assemble() does
and for the same reason.

The identity case is the gate: re-encoding with a mapping that changes
nothing has to give back the bytes that were read, for every instruction in
the corpus. That is checkable before anything depends on the result, which
is the only moment it is free.
"""

from dataclasses import dataclass

from iced_x86 import OpKind
from iced_x86 import Decoder
from iced_x86 import Encoder
from iced_x86 import Register
from iced_x86 import Register_
from iced_x86 import MemorySizeExt

from qbopt.ir import ROOT
from qbopt.declen import INFO
from qbopt.declen import Insn
from qbopt.declen import WRITES
from qbopt.declen import BITNESS

# How many operands iced will report a register for. Nothing BC emits has
# more, and asking past it raises rather than returning nothing.
MAX_OPERANDS = 5


@dataclass(frozen=True, slots=True)
class Emitted:
    """The re-encoded instruction, and where its displacement ended up."""

    code: bytes
    displacement_at: int | None  # offset within `code`, for moving a fixup

    @property
    def length(self) -> int:
        return len(self.code)


# Each root's own 32-, 16- and 8-bit names, in one order so a register can be
# swapped for the same width of another. bx/bp/si/di have no 8-bit low half
# in the set BC uses, and are absent from the 8-bit row rather than guessed.
_ROOTS = (Register.EAX, Register.ECX, Register.EDX, Register.EBX, Register.ESI, Register.EDI)
_NARROW = (Register.AX, Register.CX, Register.DX, Register.BX, Register.SI, Register.DI)
_BYTE = (Register.AL, Register.CL, Register.DL, Register.BL, Register.NONE, Register.NONE)
_FAMILIES: dict[int, dict[Register_, Register_]] = {
    2: dict(zip(_ROOTS, _NARROW, strict=True)),
    1: dict(zip(_ROOTS, _BYTE, strict=True)),
}


def _same_width(register: Register_, target_root: Register_) -> Register_ | None:
    """`target_root`'s own name at whatever width `register` is."""
    if register in _ROOTS:
        return target_root
    for by_root in (_FAMILIES[2], _FAMILIES[1]):
        if register in by_root.values():
            found = by_root.get(target_root, Register.NONE)
            return None if found is Register.NONE else found
    return None


def with_registers(insn: Insn, mapping: dict[Register_, Register_]) -> Emitted | None:
    """`insn` with its registers remapped, or None if the form cannot hold them.

    `mapping` is between 32-bit roots. Every register operand naming a root
    in it is rewritten to that root's own name at the operand's own width;
    a memory operand's base and index are rewritten the same way, since an
    address computed from a moved value is computed from wherever it moved.
    """
    changed = insn.insn.copy()

    for index in range(min(changed.op_count, MAX_OPERANDS)):
        try:
            register = changed.op_register(index)
        except (ValueError, TypeError):
            continue
        if register == Register.NONE:
            continue
        root = ROOT.get(register, register)
        if root not in mapping or mapping[root] is root:
            continue
        found = _same_width(register, mapping[root])
        if found is None:
            return None
        changed.set_op_register(index, found)

    for attribute in ("memory_base", "memory_index"):
        register = getattr(changed, attribute)
        root = ROOT.get(register, register)
        if register == Register.NONE or root not in mapping or mapping[root] is root:
            continue
        found = _same_width(register, mapping[root])
        if found is None:
            return None
        setattr(changed, attribute, found)

    # Encoded at the address it actually sits at, not at zero. A relative
    # branch's displacement is measured from its own ip, so encoding
    # elsewhere silently retargets it -- 1,444 of the corpus's instructions
    # came back "different" for that reason alone before the identity gate
    # was pointed at the right address.
    #
    # Encoder rather than BlockEncoder: BlockEncoder rewrites a branch to
    # the shortest form that reaches, which is a better encoding and the
    # wrong answer here. BC emits `e9 0b 00` where `eb 0c` would do, and
    # shortening it changes the instruction's length -- 204 more of the
    # corpus differed for that alone. This re-encodes one instruction and
    # is not entitled to pick a different size for it.
    changed.ip = insn.at
    encoder = Encoder(BITNESS)
    try:
        encoder.encode(changed, insn.at)
    except ValueError:
        # The form will not hold that register -- an accumulator-only
        # encoding, most often `mov eax,moffs32`. See the module docstring.
        return None

    code = encoder.take_buffer()
    if len(code) != insn.length:
        # Refused rather than returned. A caller patching one instruction in
        # place wants the same bytes back; a different length is a different
        # problem and this is not the place to hide it.
        #
        # It is also what keeps an emulated x87 site from being rewritten
        # into a real one. declen.py decodes `int 34h`-`3Dh` as the float
        # instruction it stands for, so re-encoding emits the native opcode
        # -- `cd 39 04` becomes `dd 04` -- which is a different program on a
        # machine with no coprocessor. Three instructions in the corpus, all
        # of them there.
        return None

    decoder = Decoder(BITNESS, code, ip=insn.at)
    decoded = next(iter(decoder), None)
    if decoded is None:
        return None
    offsets = decoder.get_constant_offsets(decoded)
    where = offsets.displacement_offset if offsets.has_displacement else None
    return Emitted(code, where)


def with_operand(insn: Insn, root: Register_) -> Emitted | None:
    """`insn` with its memory operand read from `root` instead, or None.

    The other half of forwarding, and the half that works on a two-address
    machine. Remapping a *use* to point at the provider's register changes
    where the result lands too -- `add ax,[y]` is `ax = ax + [y]`, so
    pointing it at si makes it `si = si + ...`. Replacing the memory
    operand changes only where the second operand is read from:

        add ax,[bp-18h]  ->  add ax,si

    The destination is untouched, so nothing downstream needs rewriting,
    and it works for every consumer shape rather than only for loads. An
    accumulate keeps accumulating -- which is what makes this safe where
    deleting the instruction was not.

    Refuses when the memory operand is not exactly one of the operands, or
    when the root has no name at the operand's width, or when the result
    would not be shorter -- reading a register instead of an address should
    always save the displacement, and an encoding that does not is one this
    does not understand.
    """
    changed = insn.insn.copy()
    where = [index for index in range(min(changed.op_count, MAX_OPERANDS)) if changed.op_kind(index) == OpKind.MEMORY]
    if len(where) != 1:
        return None

    # The memory operand has to be read and not written. `cmp [x],1` is a
    # comparison and substituting it is sound; `add [x],1` writes its result
    # back to [x], and `add ax,1` writes it to ax instead -- the store to
    # memory silently disappears. Two of the twelve real-compiler
    # configurations failed on exactly that before this check.
    if any(one.access in WRITES for one in INFO.info(insn.insn).used_memory()):
        return None

    width = MemorySizeExt.size(changed.memory_size)
    if width == 4:
        found: Register_ | None = root
    else:
        found = _FAMILIES.get(width, {}).get(root, Register.NONE)
        if found is Register.NONE:
            found = None
    if found is None:
        return None

    changed.set_op_kind(where[0], OpKind.REGISTER)
    changed.set_op_register(where[0], found)
    changed.ip = insn.at

    encoder = Encoder(BITNESS)
    try:
        encoder.encode(changed, insn.at)
    except ValueError:
        return None

    code = encoder.take_buffer()
    if len(code) >= insn.length:
        return None
    return Emitted(code, None)
