"""The pass config: which phases run, in which order, and nothing else.

LLVM's `TargetPassConfig`, whose `addOptimizedRegAlloc()` is the list this
one mirrors. The point of having it separate is that the order is a fact
about the compiler rather than something spelled out inside whichever
function happened to need it -- and that adding a phase is adding a class
and a line here, not editing a driver.

    parse      bytes  -> Module     omf.py, module.py
    raise      Module -> MIR        mir.py
    opt        MIR    -> MIR        transform.py, in transform.pipeline()
    lower      MIR    -> LIR        lower.py
    machine    LIR    -> LIR        the list below
    write      LIR    -> bytes      backend/omfwrite.py

The machine half, against LLVM's own order:

    PHIElimination      phielim.py    out of SSA, before anything is placed
    TwoAddressInstr     twoaddr.py    x86 writes one of its own sources
    RegisterCoalescer   coalesce.py   the copies the two above just made
    SplitKit            splitkit.py   cut a range at a loop rather than spill it
    RegAlloc            allocate.py   assign; spill and assign again
    (parallel copies)   parcopy.py    a phi's moves, in an order that works
    InlineSpiller       spiller.py    inside RegAlloc's own loop
    VirtRegRewriter     allocate.py   virtual -> physical
    PrologEpilogInsert  prologue.py   reserve what the spiller took
    MachineScheduler    schedule.py   ordered physical integer operations
    FinalControlFlow    jumps.py      thread edges and choose fall-throughs

`intervals.py`, `liveness.py` and `loops.py` are analyses, not phases: they
answer questions and change nothing, which is why nothing here lists them.
"""

import argparse
from dataclasses import replace
from collections.abc import Callable

from qbopt.model import lir
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.backend import jumps
from qbopt.backend import lower
from qbopt.backend import verify
from qbopt.objectfile import omf
from qbopt.backend import farcall
from qbopt.backend import parcopy
from qbopt.backend import phielim
from qbopt.backend import twoaddr
from qbopt.model.passes import O2
from qbopt.backend import allocate
from qbopt.backend import coalesce
from qbopt.backend import omfwrite
from qbopt.backend import peephole
from qbopt.backend import prologue
from qbopt.backend import schedule
from qbopt.objectfile import module
from qbopt.optimize import transform
from qbopt.model.passes import LEVELS
from qbopt.model.passes import Options
from qbopt.backend import cpu as targets
from qbopt.backend import frame as frames
from qbopt.frontend import blocks as split
from qbopt.frontend.blocks import code_map
from qbopt.model.passes import LIRTransform


def machine(
    pinned: dict,
    frame=None,
    calls: dict | None = None,
    *,
    basic_semantics: bool = False,
    cpu: str | targets.Profile = "386",
) -> list[LIRTransform]:
    """Every phase between lowering and emission, in order."""
    from qbopt.backend import floatalloc

    target = targets.profile(cpu)
    if frame is not None and frame.native is not None:
        pinned = {**pinned, **frame.native_pins}
    return [
        farcall.FarIndirectCalls(frame if frame is not None else frames.Frame(0)),
        floatalloc.FloatAlloc(frame, basic_semantics=basic_semantics, cpu=target),
        phielim.PhiElimination(),
        twoaddr.TwoAddress(),
        coalesce.Coalescer(),
        allocate.RegAlloc(pinned, frame, cpu=target),
        # After the allocation and before anything reads the code as a
        # sequence: which moves in a phi's copy conflict is a question
        # about locations, and until the allocator has chosen them there
        # is nothing to ask.
        parcopy.ParallelCopy(),
        prologue.Prologue(frame, calls) if frame is not None else prologue.Prologue(frames.Frame(0), calls),
        peephole.Peephole(frame, cpu=target),
        # Physical CSE/DCE have exposed all safe integer work, and scheduling
        # may only move fully allocated machine occurrences.
        schedule.Scheduler(target),
        # Last, after every phase that can empty a block or expose a passage:
        # this physical order decides which explicit edge is now fall-through.
        jumps.ControlFlow(),
    ]


def optimized(
    body: mir.MirBody,
    dgroup: frozenset[int],
    calls: dict,
    cpu: str | targets.Profile = "386",
    options: Options = O2,
    *,
    blocks: list | None = None,
    found=None,
    coverage: dict | None = None,
    only: str | None = None,
    watch: "Callable[[str, mir.MirBody], None] | None" = None,
) -> mir.MirBody:
    """The MIR fixed point every driver runs, configured by target and options alone.

    A switch one frontend sets and another does not makes the same program
    compile differently by spelling: sum_three took three paths here.
    Promotion needs dominators, which an irreducible CFG -- QB's RESUME
    entering a loop -- does not have; that is a fact about the body.
    """
    from qbopt.analysis import loops

    target = targets.profile(cpu)
    if loops.irreducible(body.blocks, body.entry):
        options = replace(options, promote=False)
    return transform.applied(
        body,
        dgroup,
        calls,
        blocks=blocks,
        found=found,
        coverage=coverage,
        only=only,
        options=options,
        registers=target.register_capacity,
        call_registers=target.call_register_capacity,
        index_scales=target.address_scales,
        address_forms=target.address_forms,
        costs=target.operations,
        watch=watch,
    )


def level_option(parser: argparse.ArgumentParser) -> None:
    """GCC's spelling, `-Os` or `-O2`, as `args.options`."""

    def named(text: str) -> Options:
        try:
            return LEVELS[f"O{text}"]
        except KeyError:
            raise argparse.ArgumentTypeError(f"unknown level -O{text}; choose -Os or -O2") from None

    parser.add_argument("-O", dest="options", type=named, default=O2, metavar="{s,2}", help="optimization level")


def verified(body: lir.LirBody, stage: str, *, in_ssa: bool) -> lir.LirBody:
    """Return a well-formed body or name the phase boundary that is not.

    This is deliberately in the production driver rather than in individual
    phases: every phase is checked under the same contract, including a newly
    added one whose author did not remember to opt in.
    """
    if complaints := verify.verify(body, in_ssa=in_ssa):
        raise verify.Malformed(f"{stage}: {complaints[0]}")
    return body


def checked(body: lir.LirBody, phase: LIRTransform, *, in_ssa: bool) -> lir.LirBody:
    """Run one machine phase and verify what it returned."""
    return verified(phase.transform(body), phase.name or type(phase).__name__, in_ssa=in_ssa)


def run(
    data: bytes,
    native_fpu: bool = False,
    optimise: bool = True,
    cpu: str | targets.Profile = "386",
    options: Options = O2,
) -> tuple[bytes, str]:
    """The object, rewritten, and what happened. The input back on refusal."""
    target = targets.profile(cpu)
    records = omf.parse(data)
    found = module.of(records)
    mapped = code_map(found)
    if isinstance(mapped, str):
        return data, mapped
    blocks = split.partition(found, mapped)

    # One map for the module, and the same object reaches the raise and
    # the lowering: a contract chosen twice can be chosen differently.
    contracts = runtime.for_module(found)
    result = mir.bodies(found, blocks, contracts)
    raised = list(result)
    source = result.source
    if not raised:
        return data, "nothing to raise"

    from qbopt.optimize import rotate

    done = []
    for name, body in raised:
        if optimise:
            body = optimized(
                body,
                found.dgroup,
                found.calls,
                target,
                replace(options, promote=False, strength=False),
                blocks=blocks,
                found=found,
                coverage=source.coverage,
            )
            body = rotate.entered(body)
        low = verified(
            lower.lowered(
                name,
                body,
                found.calls,
                source.absorbed,
                contracts,
                source.coverage,
                target,
                nodes=source.nodes,
                occurrences=source.occurrences,
                hints=result.hints[body.entry],
            ),
            "lower",
            in_ssa=True,
        )
        frame = frames.of(low, found.calls)
        in_ssa = True
        for phase in machine(_pinned(low), frame, found.calls, cpu=target):
            if isinstance(phase, phielim.PhiElimination):
                in_ssa = False
            low = checked(low, phase, in_ssa=in_ssa)
        done.append(low)

    reached = frozenset(at for block in blocks for insn in block.insns for at in range(insn.at, insn.end))
    fields = frozenset(one.offset for one in omf.fixups(records) if one.seg == found.seg)
    out = omfwrite.written_bc(found, done, records, {}, mapped.tables, fields, reached, native_fpu, source=source)
    return (data, out) if isinstance(out, str) else (out, "written")


def _pinned(body) -> dict:
    """A body's pins, by value id, which is what the allocator is keyed on.

    MIR pins a value; LIR names an id. The translation is here rather than
    in the allocator because a pin is the raise's statement about the
    machine, and this is the last place that holds both forms.
    """
    return {value.id: register for value, register in (getattr(body, "pins", None) or {}).items()}
