"""
An object whose code segment this pass wrote, rather than edited.

Every other path in qbopt patches BC's own bytes in place -- a region here,
an instruction there -- and lives with the layout BC chose. This one does
not: `layout.rebuild` places every body afresh and `relocate.as_records`
writes the records to match, so chunk boundaries, branch displacements and
fixup offsets are all produced rather than preserved.

That is what per-body splicing could not do. Fitting a laid-out body back
into BC's own layout works for 1 of the corpus's 171 bodies -- the rest
cross a LEDATA boundary, are not contiguous, or are branched into from
outside -- and none of those apply when the boundaries are yours to choose.

Refuses far more than it accepts, and says why each time. A body holding an
op select.py cannot emit, or data BC put between the instructions, refuses
the whole object: half this pass's code and half BC's is not something
anything downstream could reason about.
"""

from enum import StrEnum
from dataclasses import dataclass
from collections.abc import Callable

from qbopt.model import mir
from qbopt.objectfile import omf
from qbopt.objectfile import module
from qbopt.abi import runtime
from qbopt.optimize import transform
from qbopt.frontend import blocks as split
from qbopt.frontend.blocks import code_map

type Watch = Callable[[str, str | None, object], None]


class Emission(StrEnum):
    """Which emitter produced an object, or that none did.

    `rebuilt` says only whether it worked, and two emitters are coming:
    the one below, from MIR, and objwrite.py's, from LIR. A caller that
    has to tell them apart -- one whose output is a program rather than a
    body, and must not be raised again -- cannot ask a boolean, and "it
    worked" would treat a fallback as the other's own output.
    """

    LIR = "lir"
    MIR = "mir"
    REFUSED = "refused"


@dataclass(frozen=True)
class Emitted:
    """An object, what made it, and what it said about doing so."""

    data: bytes
    outcome: Emission
    reason: str
    # Retained for readers of historical MIR-emission results. Production
    # has one backend now; a refusal is in `reason` and preserves the input.
    fallback_reason: str | None = None


def emitted(
    data: bytes,
    optimise: bool = True,
    native_fpu: bool = False,
    only: str | None = None,
    watch: Watch | None = None,
    cpu: str = "386",
    basic_semantics: bool = False,
    bounds_checks: bool = False,
) -> Emitted:
    """The object rewritten, and which emitter did it.

    `watch(stage, name, low)` is called after lowering and after each
    machine phase of *this* run, and once with the route the bytes came
    from. Rule 4 asks for a file per stage, and a tool that recomputes the
    phases to get them dumps a different program: its allocation is not the
    one that produced the object, so the first bad transition is not in it.
    """
    if basic_semantics and native_fpu:
        raise ValueError("--basic-semantics cannot be combined with --native-fpu")
    out, why, _ = _rebuilt(data, optimise, native_fpu, only, watch, cpu, basic_semantics, bounds_checks)
    if why != REBUILT:
        return Emitted(out, Emission.REFUSED, why)
    return Emitted(out, Emission.LIR, why)


def rebuilt(
    data: bytes,
    optimise: bool = True,
    native_fpu: bool = False,
    only: str | None = None,
    *, basic_semantics: bool = False, bounds_checks: bool = False,
) -> tuple[bytes, str]:
    """`emitted`, as every caller already reads it."""
    got = emitted(data, optimise, native_fpu, only, basic_semantics=basic_semantics, bounds_checks=bounds_checks)
    return got.data, got.reason


def _rebuilt(
    data: bytes,
    optimise: bool = True,
    native_fpu: bool = False,
    only: str | None = None,
    watch: Watch | None = None,
    cpu: str = "386",
    basic_semantics: bool = False,
    bounds_checks: bool = False,
) -> tuple[bytes, str, str | None]:
    """The object with its code segment rewritten, and what happened.

    Returns the input unchanged where anything refuses, so a caller can use
    this as a transform without deciding first whether it will work.
    """
    records = omf.parse(data)
    found = module.of(records)
    if found is None:
        return data, "the module has no code segment", None
    mapped = code_map(found)
    if isinstance(mapped, str):
        return data, mapped, None

    blocks = split.partition(found, mapped)
    # One map for the whole module, and the same object reaches the raise
    # and the lowering: a contract chosen twice can be chosen differently.
    contracts = runtime.for_module(found)
    bodies = list(mir.bodies(found, blocks, contracts, basic_semantics=basic_semantics,
                            bounds_checks=bounds_checks))
    if not bodies:
        return data, "no bodies were raised", None
    if not bounds_checks:
        retained = next((op for _, body in bodies for block in body.blocks for op in block.ops
                         if op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$HARY"), None)
        if retained is not None:
            return data, f"unchecked array lowering unsupported at {retained.at:#x} (B$HARY)", None

    # Optimised as values before being written as bytes. transform.py's own
    # docstring has why every original byte still has to be accounted for
    # after a deletion; `optimise=False` emits the body exactly as raised,
    # which is what a caller bisecting a layout question wants.
    if optimise:
        # Widening is not a MIR pass and is no longer in the list. It
        # recognises an idiom -- a long written as two halves joined by a
        # carry -- and writes the one 32-bit operation that replaces it,
        # which is machine form: 95 register references, all of them BC's
        # ax:dx convention. Recognition belongs at the raise and emission
        # below the boundary; until the two are separated it runs here,
        # after every pass and before lowering, which is where it ran
        # anyway and is where rule 5 puts it.
        def one(name: str, body: mir.MirBody) -> mir.MirBody:
            # `only="widen"` is the step on its own, which tools/stages.py
            # asks for; every other name selects a pass and leaves widening
            # out, so the two can be diffed apart.
            if only == "widen":
                return transform.widened(body)
            done = transform.applied(
                body,
                found.dgroup,
                found.calls,
                blocks=blocks,
                found=found,
                only=only,
                watch=(lambda stage, state: watch(f"mir-{stage}", name, state)) if watch is not None else None,
            )
            if only is None:
                done = transform.widened(done)
                if watch is not None:
                    watch("mir-widen", name, done)
            return done

        bodies = [(name, one(name, body)) for name, body in bodies]

    # Every byte the decoder walked into, so layout.py can tell a gap it may
    # carry from one that is real code it simply did not raise.
    reached = frozenset(at for block in blocks for insn in block.insns for at in range(insn.at, insn.end))
    fields = frozenset(one.offset for one in omf.fixups(records) if one.seg == found.seg)
    short = _through_lir(found, records, blocks, bodies, mapped, fields, reached, native_fpu, contracts, watch, cpu)
    if not isinstance(short, str):
        if watch is not None:
            watch("route", None, "the LIR emitter wrote these bytes")
        return short, REBUILT, None
    if watch is not None:
        watch("route", None, f"the LIR emitter refused ({short}); the input is unchanged")
    return data, short, None


def _through_lir(
    found: module.Module,
    records: list[omf.Record],
    blocks: list[split.Block],
    bodies: list[tuple[str, mir.MirBody]],
    mapped: split.CodeMap,
    fields: frozenset[int],
    reached: frozenset[int],
    native_fpu: bool,
    contracts: dict[int, runtime.Contract],
    watch: Watch | None = None,
    cpu: str = "386",
) -> bytes | str:
    """Every body lowered, placed and written, or why one could not be.

    `qbopt/flow.py` names the phases and their order; this runs them and
    hands the result to objwrite.py, which converges on the same layout
    and relocation the MIR path uses. A refusal comes back as its own
    words rather than as a fallback taken quietly -- what refuses and why
    is the measurement the phase order is judged by.
    """
    from qbopt import flow
    from qbopt.backend import lower
    from qbopt.backend import parcopy
    from qbopt.backend import spiller
    from qbopt.backend import allocate
    from qbopt.objectfile import objwrite
    from qbopt.backend import frame as frames
    from qbopt.backend import pointers
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr, Space

    pointer_model = None
    if any(op.kind is mir.Kind.PTR_OFFSET or any(ref.pointer for ref in (*op.loads, *op.stores))
           for _, body in bodies for block in body.blocks for op in block.ops):
        records, index = omf.with_external(records, "b$HugeShift")
        pointer_model = pointers.Model(ir.Mem(Addr(Space.EXTERNAL, 0, index), 1, disp_width=2))

    done = []
    for name, body in bodies:
        try:
            low = lower.lowered(
                name,
                body,
                found.calls,
                found.absorbed,
                contracts,
                found.coverage,
                cpu,
                pointer_model=pointer_model,
            )
            if watch is not None:
                watch("lowered", name, low)
            frame = frames.of(low, found.calls)
            for phase in flow.machine(flow._pinned(low), frame, found.calls):
                low = phase.transform(low)
                if watch is not None:
                    watch(phase.name, name, low)
        except (lower.Unlowered, mir.Unraisable, frames.Refused) as short:
            # A contract this cannot honour, or an operand no encoding
            # covers. Named here for the same reason as the four below:
            # this body falls back to BC's own layout instead of taking
            # the whole module down.
            return f"{name}: {type(short).__name__}: {short}"
        except allocate.Spilled as short:
            return f"{name}: Spilled: {short}"
        except allocate.Unplaced as short:
            return f"{name}: Unplaced: {short}"
        except spiller.Simultaneous as short:
            # A phi's copy with both ends in slots: `mov [bp-2],[bp-4]` is
            # not an instruction, and splitting it puts an ungrouped one
            # inside a group whose moves happen at once. Named, so this
            # falls back to BC's own layout rather than escaping as a
            # crash -- five objects did.
            return f"{name}: Simultaneous: {short}"
        except parcopy.Tangled as short:
            # A phi's copies that all read each other's destinations need a
            # temporary this does not have yet. Malformed is deliberately
            # not caught: it says something that is not a move was put in a
            # copy group, which is a bug here rather than a body this
            # cannot place.
            return f"{name}: Tangled: {short}"
        done.append(low)
    return objwrite.written(found, done, records, {}, mapped.tables, fields, reached, native_fpu)


REBUILT = "rebuilt"
