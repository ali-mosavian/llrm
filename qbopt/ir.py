"""
Total decode: every instruction in a Body becomes a Node, in order.

lift.py's own value tracker treats anything it does not recognise as a wall
-- "anything not recognised invalidates both pairs, because an instruction
this does not understand may write either of them" (lift.py's own module
docstring). That policy is correct as a default and, per docs/residue.md's
patterns G and H, wrong specifically where the unrecognised instruction is
one this pass itself just emitted (calls.py's own restore idiom) and whose
real effect is knowable. This module is what makes that fixable without
redesign: every instruction gets a real, typed node with an iced-derived
def/use/flags/memory effect, so a later pass can reason about an Opaque node
generically instead of only ever falling off a cliff.

Known shapes -- lift.classify()'s six single-instruction long-pair forms,
a far call a fixup names, and calls.py's own three-instruction "restore"
idiom -- get a typed node. Everything else is Opaque, deliberately: commit 3
is where real optimisation starts, and this module's whole job is proving
the round trip, not building out a rich op vocabulary early.

The emitter never reads a node's semantic fields, only its own byte span --
`module.code[node.at:node.end]` via `span()`, for every node kind, always.
That is what makes "decode a Body, re-emit it, compare to the original"
hold by construction: a wrong Node type or a wrong Effects field can never
corrupt commit 1's own output, because emission never consults them. It can
only corrupt what a future commit computes from the IR, which is that
commit's own gate to hold, not this one's.
"""

from enum import StrEnum
from dataclasses import dataclass

from iced_x86 import Register
from iced_x86 import Register_

from qbopt.flags import Flag
from qbopt.lift import FIXUP
from qbopt.declen import INFO
from qbopt.declen import Insn
from qbopt.extent import Body
from qbopt.module import Addr
from qbopt.blocks import Block
from qbopt.declen import READS
from qbopt.lift import Decoded
from qbopt.declen import WRITES
from qbopt.lift import Resolver
from qbopt.lift import classify
from qbopt.module import Module
from qbopt.blocks import CodeMap
from qbopt.flags import CLOBBERS
from qbopt.blocks import code_map
from qbopt.extent import Partition
from qbopt.flags import written_by
from qbopt.blocks import INLINE_TABLE
from qbopt.lift import operand as long_operand
from qbopt.extent import partition as body_partition
from qbopt.blocks import partition as block_partition

# The 32-bit root of every general-purpose register this pass ever reasons
# about. AL/AH/AX/EAX are all "does this touch the ax pair" -- reporting
# whichever sub-register iced happened to decode would push "does a write to
# eax kill dx" (it does not -- that is the entire reason calls.py's restore
# exists) onto every consumer instead of deciding it once, here.
ROOT = {
    Register.AL: Register.EAX,
    Register.AH: Register.EAX,
    Register.AX: Register.EAX,
    Register.EAX: Register.EAX,
    Register.BL: Register.EBX,
    Register.BH: Register.EBX,
    Register.BX: Register.EBX,
    Register.EBX: Register.EBX,
    Register.CL: Register.ECX,
    Register.CH: Register.ECX,
    Register.CX: Register.ECX,
    Register.ECX: Register.ECX,
    Register.DL: Register.EDX,
    Register.DH: Register.EDX,
    Register.DX: Register.EDX,
    Register.EDX: Register.EDX,
    Register.SI: Register.ESI,
    Register.ESI: Register.ESI,
    Register.DI: Register.EDI,
    Register.EDI: Register.EDI,
    Register.BP: Register.EBP,
    Register.EBP: Register.EBP,
    Register.SP: Register.ESP,
    Register.ESP: Register.ESP,
}


def root(register: Register_) -> Register_:
    return ROOT.get(register, register)


@dataclass(frozen=True, slots=True)
class Effects:
    """One instruction's (or idiom's) real, conservative effect.

    None for `defs`/`uses` means "assume any register" -- the answer for a
    call or interrupt, whose real effect is the callee's, not what iced's
    per-instruction info reports for the call site itself (flags.written_by
    already treats a call's flags this way; the same conservatism applies to
    registers and memory here, for the same reason).
    `memory` is a resolvable static address, or None if `touches_memory` and
    the address is not known -- conservative, must be assumed to alias
    anything.
    """

    defs: frozenset[Register_] | None
    uses: frozenset[Register_] | None
    flags_written: Flag
    touches_memory: bool
    memory: Addr | None


NO_EFFECT = Effects(frozenset(), frozenset(), Flag.NONE, False, None)


def _register_effects(insn: Insn) -> tuple[frozenset[Register_], frozenset[Register_]]:
    """Which roots this instruction may write, and which it may read.

    A write iced reports against a sub-register (`mov ax,cx` writes AX, not
    EAX) is a partial write of its root: the bits it does not touch survive,
    so the root also belongs in `uses` -- not just `defs` -- or a consumer
    would think the whole register was freshly defined. `lift.py`'s own
    MOVE comment names exactly this ("mov ax,cx writes sixteen bits and
    leaves the top half of eax stale"), and it is the entire reason
    calls.py's restore idiom exists at all.
    """
    used = list(INFO.info(insn.insn).used_registers())
    defs: set[Register_] = set()
    uses: set[Register_] = set()
    for one in used:
        target = root(one.register)
        if one.access in WRITES:
            defs.add(target)
            if target != one.register:
                uses.add(target)
        if one.access in READS:
            uses.add(target)
    return frozenset(defs), frozenset(uses)


def instruction_effects(insn: Insn, resolve: Resolver) -> Effects:
    """The conservative, iced-derived effect of one real instruction."""
    if insn.flow in CLOBBERS:
        return Effects(None, None, written_by(insn), True, None)
    defs, uses = _register_effects(insn)
    touches_memory = bool(list(INFO.info(insn.insn).used_memory()))
    memory = long_operand(insn, resolve) if touches_memory else None
    return Effects(defs, uses, written_by(insn), touches_memory, memory)


@dataclass(frozen=True, slots=True)
class Opaque:
    """An instruction with no shape this pass recognises by name."""

    insn: Insn
    effects: Effects


@dataclass(frozen=True, slots=True)
class Long:
    """One of lift.classify()'s six single-instruction long-pair shapes."""

    insn: Insn
    decoded: Decoded
    effects: Effects


@dataclass(frozen=True, slots=True)
class Call:
    """A far call whose target a fixup names (module.calls) -- a runtime
    routine or a user external, not only the ones calls.py knows how to
    absorb (calls.py's own LEFT_FIRST vocabulary is deliberately not
    imported here: "the target is named" is all this layer asserts)."""

    insn: Insn
    name: str
    effects: Effects


# The pair each restore idiom's own bytes belong to, per lift.FIXUP -- reused
# rather than re-declared, since the bytes have to be exactly these to be
# this idiom at all.
RESTORE_EFFECTS = {
    # Net stack effect is nothing: sp returns to where it started and
    # nothing outside this idiom ever reads the stack cells it transiently
    # used, so touches_memory is False even though push/pop individually
    # touch memory. Writes no flags at all -- lift.py chose this idiom over
    # `shr` specifically because it does not, and tests/test_flags.py
    # asserts that transparency. Both halves' `pop` is a partial write of
    # its own root (`pop ax` touches only eax's low 16 bits), so -- per
    # _register_effects' own rule for a partial write -- both roots belong
    # in `uses` too, not only `defs`.
    0: Effects(
        frozenset({Register.EAX, Register.EDX}), frozenset({Register.EAX, Register.EDX}), Flag.NONE, False, None
    ),
    1: Effects(
        frozenset({Register.ECX, Register.EBX}), frozenset({Register.ECX, Register.EBX}), Flag.NONE, False, None
    ),
}


@dataclass(frozen=True, slots=True)
class Restore:
    """calls.py's own `push e?x / pop ?x / pop ?x` idiom, byte-identical to
    lift.FIXUP[pair] -- puts a widened value's high half back where BC's
    un-widened code reads it. BC never emits this; it exists in real code
    only after this pass's own absorption has already run once, so it is
    exercised by re-decoding a rewritten object, not by the untouched
    110-fixture corpus."""

    at: int
    end: int
    pair: int
    effects: Effects


class TableKind(StrEnum):
    # B$OGTA's own inline data: real jump targets, per extent._table_targets.
    JUMP = "jump"
    # anything else a TABLE-ending block owns -- the /X RESUME map is the
    # one blocks.py names, data nothing jumps into.
    MAP = "map"


@dataclass(frozen=True, slots=True)
class Data:
    """Bytes a Body owns that are not instructions at all -- always an
    inline table appended to a body's range by extent.py's own _ranges()."""

    at: int
    end: int
    kind: TableKind
    entries: tuple[int, ...]
    effects: Effects


type Node = Opaque | Long | Call | Restore | Data


def span(node: Node) -> tuple[int, int]:
    match node:
        case Opaque(insn=insn) | Long(insn=insn) | Call(insn=insn):
            return insn.at, insn.end
        case Restore(at=at, end=end) | Data(at=at, end=end):
            return at, end


def emit(module: Module, nodes: tuple[Node, ...]) -> bytes:
    """A node list's own bytes, verbatim -- never reconstructed, always
    sliced from the original code by each node's own span. See this
    module's own docstring for why that is deliberate."""
    return b"".join(module.code[lo:hi] for lo, hi in map(span, nodes))


def _restore_at(module: Module, insns_by_at: dict[int, Insn], at: int, hi: int) -> Restore | None:
    """A restore idiom starting exactly at `at`, or None.

    Guarded on real instruction starts, not just a byte match: `at+2` (pop
    lo16) and `at+3` (pop hi16) must themselves be instruction boundaries
    inside this range, so a coincidental byte match cannot claim to be this
    idiom while actually straddling something else.
    """
    if at + 4 > hi:
        return None
    for pair, pattern in FIXUP.items():
        if module.code[at : at + 4] != pattern:
            continue
        second, third = insns_by_at.get(at + 2), insns_by_at.get(at + 3)
        if second is None or second.end != at + 3 or third is None or third.end != at + 4:
            continue
        return Restore(at, at + 4, pair, RESTORE_EFFECTS[pair])
    return None


def _table_node(module: Module, last: Node | None, lo: int, hi: int) -> Data:
    kind = TableKind.MAP
    if isinstance(last, Call) and last.insn.end == lo and last.name in INLINE_TABLE:
        kind = TableKind.JUMP
    # `lo` is a real fixup site for a MAP table (blocks.unexplained_tables()
    # returns the first fixup offset itself as its span's own start) but
    # never one for a JUMP table (`lo` there is B$OGTA's own count byte, one
    # short of its first offset16 entry) -- `<=` is correct for both, since
    # a JUMP table's `lo` is simply never a key in module.operands.
    entries = tuple(sorted(at for at in module.operands if lo <= at < hi))
    return Data(lo, hi, kind, entries, NO_EFFECT)


def decode_body(module: Module, mapped: CodeMap, blocks: list[Block], body: Body) -> tuple[Node, ...]:
    """Every byte of `body`'s own ranges, as ordered Nodes.

    Instruction boundaries come from `blocks` (already the exact, reachability-
    proven decode blocks.py produced) rather than a second, independent
    decode walk -- one source of truth for "where an instruction is", the
    same discipline extent.py itself follows.
    """
    insns_by_at = {insn.at: insn for block in blocks for insn in block.insns}
    tables_by_start = dict(mapped.tables)

    nodes: list[Node] = []
    last: Node | None = None
    for lo, hi in body.ranges:
        at = lo
        while at < hi:
            if at in tables_by_start:
                node: Node = _table_node(module, last, at, tables_by_start[at])
                nodes.append(node)
                last = node
                at = node.end
                continue
            restore = _restore_at(module, insns_by_at, at, hi)
            if restore is not None:
                nodes.append(restore)
                last = restore
                at = restore.end
                continue
            insn = insns_by_at[at]
            effects = instruction_effects(insn, module.resolve)
            if insn.at in module.calls:
                node = Call(insn, module.calls[insn.at], effects)
            elif (decoded := classify(insn, module.resolve)) is not None:
                node = Long(insn, decoded, effects)
            else:
                node = Opaque(insn, effects)
            nodes.append(node)
            last = node
            at = insn.end
    return tuple(nodes)


@dataclass(frozen=True, slots=True)
class BodyIR:
    body: Body
    nodes: tuple[Node, ...]


def decode_module(module: Module) -> tuple[BodyIR, ...] | str:
    """Every body of `module`, total-decoded -- or why it could not be."""
    mapped = code_map(module)
    if isinstance(mapped, str):
        return mapped
    found = body_partition(module)
    if isinstance(found, str):
        return found
    if not found.complete:
        return _incomplete(found)
    blocks = block_partition(module, mapped)
    return tuple(BodyIR(body, decode_body(module, mapped, blocks, body)) for body in found.bodies)


def _incomplete(found: Partition) -> str:
    return f"{len(found.unexplained)} unexplained range(s), {len(found.conflicts)} conflicting"
