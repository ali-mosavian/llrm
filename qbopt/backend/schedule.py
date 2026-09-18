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

from qbopt.backend import cpu as targets
from qbopt.model import ir
from qbopt.model import lir
from qbopt.model.passes import LIRTransform


_GENERAL = frozenset(
    {Register.EAX, Register.EBX, Register.ECX, Register.EDX, Register.ESI, Register.EDI, Register.EBP}
)


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
        }
        or not what.dests
        or any(not isinstance(where, ir.Reg) for where in what.dests)
        or any(not isinstance(where, (ir.Reg, ir.Imm)) for where in what.sources)
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
    if what.name == "imul":
        return "imul_r32" if any(where.width == 4 for where in (*what.dests, *what.sources)) else "mul_r16"
    if what.name in {"mov", "movsx", "movzx"}:
        if what.name == "movzx":
            return "movzx"
        return "mov_ri" if any(isinstance(where, ir.Imm) for where in what.sources) else "mov_rr"
    if what.name in {"shl", "shr", "sar", "rol", "ror"}:
        return "shift_ri"
    if what.name in {"cwd", "cdq"}:
        return "cdq"
    if what.op in {ir.Operation.BINARY, ir.Operation.UNARY, ir.Operation.COMPARE, ir.Operation.FUNNEL}:
        return "alu_rr"
    return "unknown"


def _latency(one: lir.Insn, cpu: targets.Profile) -> int:
    form = _form(one)
    try:
        return max(1, cpu.latency(form))
    except KeyError:
        return 1


def _ordered(window: list[lir.Insn], cpu: targets.Profile) -> list[lir.Insn]:
    """List-schedule one side-effect-free window by lanes and measured latency."""
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
            ready_at[user] = max(ready_at[user], done)
        # The listing has no explicit no-ops.  One issue slot was consumed;
        # skipped cycles represent hardware waiting for a dependency.
        clock += 1
    return emitted


def scheduled(body: lir.LirBody, cpu: str | targets.Profile = "386") -> lir.LirBody:
    """Hide safe dependency gaps for an out-of-order profile, else retain order."""
    target = targets.profile(cpu)
    # P5 needs U/V-pipe pairing constraints, not merely a generic width of
    # two; 386/486 are in-order.  Retaining their established listing is more
    # correct than pretending either has the later-core scheduling model.
    if target.in_order:
        return body
    blocks = []
    changed = False
    for block in body.blocks:
        out: list[lir.Insn] = []
        window: list[lir.Insn] = []

        def flush() -> None:
            nonlocal changed, window
            if window:
                ordered = _ordered(window, target)
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
