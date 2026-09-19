"""Latency-aware ordering of safe, already-allocated machine instructions.

This is deliberately narrower than an instruction scheduler in a flat 32-bit
compiler.  The medium-model output has segment state, far calls, source-map
anchors, and precise x87 exception ordering.  Until all of those have a
complete dependency model, they are boundaries.  Within the remaining
integer register/immediate windows, physical register and flag lanes are the
complete dependency graph, so a later independent operation can fill the
latency of an earlier producer without changing memory or ABI behaviour.
"""

from dataclasses import replace

from iced_x86 import Register
from iced_x86 import RegisterExt

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import cpu as targets
from qbopt.objectfile.module import Space
from qbopt.model.passes import LIRTransform

_GENERAL = frozenset({Register.EAX, Register.EBX, Register.ECX, Register.EDX, Register.ESI, Register.EDI, Register.EBP})


class Scheduler(LIRTransform):
    """Hide measured dependency latency where the complete hardware state is known."""

    name = "schedule"

    def __init__(self, cpu: str | targets.Profile = "386"):
        self.cpu = targets.profile(cpu)

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return scheduled(body, self.cpu)


def _safe(one: lir.Insn) -> "tuple[frozenset, frozenset] | None":
    """Return physical reads/writes for a freely movable integer occurrence.

    Memory, stack/segment state, symbolic operands, calls, control transfer,
    x87, and source-map-sensitive allocator artifacts are boundaries.  This
    is a proof boundary, not a list of currently inconvenient cases: every
    form left inside has only GPR/flag state represented by `_effects`.
    """
    from qbopt.backend.peephole import _lanes
    from qbopt.backend.peephole import _register_effects

    what = one.what
    if (
        what is None
        or what.op
        not in {
            ir.Operation.MOVE,
            ir.Operation.BINARY,
            ir.Operation.UNARY,
            ir.Operation.MULTIPLY,
            ir.Operation.COMPARE,
            ir.Operation.EXTEND,
            ir.Operation.FUNNEL,
            ir.Operation.ADDRESS,
        }
        or not what.dests
        or any(not isinstance(where, ir.Reg) for where in what.dests)
        or (
            what.op is not ir.Operation.ADDRESS
            and any(not isinstance(where, (ir.Reg, ir.Imm)) for where in what.sources)
        )
        or (
            what.op is ir.Operation.ADDRESS
            and (
                len(what.sources) != 1
                or not isinstance(what.sources[0], ir.Address)
                or what.sources[0].addr is None
                or what.sources[0].addr.space is not Space.FRAME
            )
        )
        or any(isinstance(where, ir.Imm) and where.address is not None for where in what.sources)
        or one.clobbers
        or one.clobbers_high
        or one.requires
        or one.delivers
        or one.spread
        or one.group is not None
        or one.symbol is not None
        or one.frame_adjust
        or one.spill_reload
        or one.spill_store
        or one.rematerialized
        or getattr(one.op, "barrier", False)
    ):
        return None
    registers = [where.register for where in (*what.dests, *what.sources) if isinstance(where, ir.Reg)]
    if what.op is ir.Operation.ADDRESS:
        address = what.sources[0]
        assert isinstance(address, ir.Address)
        registers.extend(register for register in (address.through, address.index) if register is not Register.NONE)
    if any(RegisterExt.full_register32(register) not in _GENERAL for register in registers):
        return None
    effects = _register_effects(one, flags=True)
    if effects is None:
        return None
    reads, writes = map(frozenset, effects)
    if any(lane[0] is not Register.NONE and lane not in _lanes(lane[0]) for lane in reads | writes):
        return None
    return reads, writes


def _form(one: lir.Insn) -> str:
    """The audited profile key for a safe selected form, or ``unknown``."""
    assert one.what is not None
    what = one.what
    if what.op is ir.Operation.ADDRESS:
        return "lea"
    if what.name == "imul":
        return "imul_r32" if any(where.width == 4 for where in (*what.dests, *what.sources)) else "mul_r16"
    if what.name in {"mov", "movsx", "movzx"}:
        if what.name == "movzx":
            return "movzx"
        return "mov_ri" if any(isinstance(where, ir.Imm) for where in what.sources) else "mov_rr"
    if what.name in {"shl", "shr", "sar", "rol", "ror"} or what.op is ir.Operation.FUNNEL:
        return "shift_ri"
    if what.name in {"cwd", "cdq"}:
        return "cdq"
    if what.op in {ir.Operation.BINARY, ir.Operation.UNARY, ir.Operation.COMPARE}:
        return "alu_rr"
    return "unknown"


def _latency(one: lir.Insn, cpu: targets.Profile) -> int:
    form = _form(one)
    try:
        return max(1, cpu.latency(form))
    except KeyError:
        return 1


def _partial_merge_delay(window: list[lir.Insn], producer: int, consumer: int, cpu: targets.Profile) -> int:
    """The profile's 16/8-to-32-bit merge delay on one real dependency edge.

    A full write of the same root in between replaces the partial value, so
    the later 32-bit read does not need the old upper bytes and has no merge
    dependency.  The narrow operand check is intentionally syntactic: this
    post-allocation phase knows exact physical roots, not source values.
    """
    if not cpu.partial_register_stall:
        return 0
    before, after = window[producer].what, window[consumer].what
    assert before is not None and after is not None
    partial = {
        RegisterExt.full_register32(where.register)
        for where in before.dests
        if isinstance(where, ir.Reg) and where.width < 4 and RegisterExt.full_register32(where.register) in _GENERAL
    }
    wide = {
        RegisterExt.full_register32(where.register)
        for where in after.sources
        if isinstance(where, ir.Reg) and where.width == 4 and RegisterExt.full_register32(where.register) in partial
    }
    if not wide:
        return 0
    for crossed in window[producer + 1 : consumer]:
        what = crossed.what
        if what is not None and any(
            isinstance(where, ir.Reg) and where.width == 4 and RegisterExt.full_register32(where.register) in wide
            for where in what.dests
        ):
            return 0
    return cpu.partial_register_stall


def _graph(window: list[lir.Insn]) -> "tuple[list[tuple[frozenset, frozenset]], list[set[int]], list[set[int]]]":
    effects = [_safe(one) for one in window]
    assert all(one is not None for one in effects)
    needs = [set() for _ in window]
    users = [set() for _ in window]
    for left, (reads, writes) in enumerate(effects):
        assert reads is not None and writes is not None
        for right in range(left + 1, len(window)):
            later_reads, later_writes = effects[right]
            # RAW, WAR and WAW including FLAGS.  The direction is the
            # original program order; no independence is inferred from a
            # mnemonic or from a source-level value that no longer exists.
            if writes & (later_reads | later_writes) or reads & later_writes:
                users[left].add(right)
                needs[right].add(left)
    return effects, needs, users


def _pair_class(one: lir.Insn) -> str:
    """The audited non-MMX Pentium U/V category for a safe register form.

    GCC's local Pentium model supplies the rule, not an instruction listing:
    operand/address-size prefixes use U only, immediate/displacement forms
    and multiply use neither pairing slot, and the remaining register ALU or
    move forms can issue in either pipe.  `_safe` has already ruled out memory
    and all forms whose category is not complete here.
    """
    from qbopt.backend import select

    assert one.what is not None
    encoded = select.emit(one.what)
    if encoded is None:
        return "np"
    # GCC's Pentium description marks scalar SHLD/SHRD `pent_pair=np` even
    # though their 32-bit form also carries the otherwise-U-only 66h prefix.
    if one.what.op is ir.Operation.FUNNEL:
        return "np"
    if encoded.code[:1] in {b"\x66", b"\x67", b"\xf2", b"\xf3"}:
        return "u"
    if one.what.name == "imul" or any(isinstance(where, ir.Imm) for where in one.what.sources):
        return "np"
    if one.what.name in {"shl", "shr", "sar", "rol", "ror"}:
        return "u"
    return "uv"


def _pentium_ordered(window: list[lir.Insn], cpu: targets.Profile) -> list[lir.Insn]:
    """Issue independent audited U/V pairs in an in-order Pentium listing."""
    _effects, needs, users = _graph(window)
    ready_at = [0] * len(window)
    left = set(range(len(window)))
    emitted: list[lir.Insn] = []
    clock = 0
    while left:
        ready = [index for index in left if not needs[index] and ready_at[index] <= clock]
        if not ready:
            clock = min(ready_at[index] for index in left if not needs[index])
            continue
        classes = {index: _pair_class(window[index]) for index in ready}
        # A U-only form can pair only as the first instruction, while an
        # ordinary form can be placed in U or V.  Prefer a candidate that
        # actually makes a pair; otherwise preserve source order.
        pair_starters = [
            index
            for index in ready
            if classes[index] in {"u", "uv"} and any(other != index and classes[other] == "uv" for other in ready)
        ]
        first = min(pair_starters or ready, key=lambda index: index)
        emitted.append(window[first])
        left.remove(first)
        done = clock + _latency(window[first], cpu)
        for user in users[first]:
            needs[user].remove(first)
            ready_at[user] = max(ready_at[user], done + _partial_merge_delay(window, first, user, cpu))

        # The V slot may only receive a fully pairable form.  Its dependencies
        # were already ready before the U-slot occurrence, so it cannot read
        # a same-cycle result from that occurrence.
        seconds = [index for index in ready if index in left and classes[index] == "uv"]
        if seconds and classes[first] in {"u", "uv"}:
            second = min(seconds)
            emitted.append(window[second])
            left.remove(second)
            done = clock + _latency(window[second], cpu)
            for user in users[second]:
                needs[user].remove(second)
                ready_at[user] = max(ready_at[user], done + _partial_merge_delay(window, second, user, cpu))
        clock += 1
    return emitted


def _ordered(window: list[lir.Insn], cpu: targets.Profile) -> list[lir.Insn]:
    """List-schedule one side-effect-free window by lanes and measured latency."""
    _effects, needs, users = _graph(window)
    ready_at = [0] * len(window)
    left = set(range(len(window)))
    emitted: list[lir.Insn] = []
    clock = 0
    while left:
        ready = [index for index in left if not needs[index] and ready_at[index] <= clock]
        if not ready:
            clock = min(ready_at[index] for index in left if not needs[index])
            continue
        # Long producers first.  Ties retain source order, making the
        # scheduler deterministic and avoiding invented P5 pairing claims.
        chosen = max(ready, key=lambda index: (_latency(window[index], cpu), -index))
        emitted.append(window[chosen])
        left.remove(chosen)
        done = clock + _latency(window[chosen], cpu)
        for user in users[chosen]:
            needs[user].remove(chosen)
            ready_at[user] = max(ready_at[user], done + _partial_merge_delay(window, chosen, user, cpu))
        # The listing has no explicit no-ops.  One issue slot was consumed;
        # skipped cycles represent hardware waiting for a dependency.
        clock += 1
    return emitted


def scheduled(body: lir.LirBody, cpu: str | targets.Profile = "386") -> lir.LirBody:
    """Hide safe dependency gaps for an out-of-order profile, else retain order."""
    target = targets.profile(cpu)
    # 386/486 are in-order.  P5's U/V pairing is its own audited profile
    # property rather than an inference from issue width.
    if target.in_order and not target.pentium_pairing:
        return body
    blocks = []
    changed = False
    for block in body.blocks:
        out: list[lir.Insn] = []
        window: list[lir.Insn] = []

        def flush() -> None:
            nonlocal changed, window
            if window:
                ordered = _pentium_ordered(window, target) if target.pentium_pairing else _ordered(window, target)
                changed |= ordered != window
                out.extend(ordered)
                window = []

        for one in block.insns:
            if _safe(one) is None:
                flush()
                out.append(one)
            else:
                window.append(one)
        flush()
        blocks.append(replace(block, insns=tuple(out)))
    return replace(body, blocks=tuple(blocks)) if changed else body
