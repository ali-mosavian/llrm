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

`intervals.py`, `liveness.py` and `loops.py` are analyses, not phases: they
answer questions and change nothing, which is why nothing here lists them.
"""

from qbopt.model import lir
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.backend import lower
from qbopt.backend import verify
from qbopt.objectfile import omf
from qbopt.backend import parcopy
from qbopt.backend import phielim
from qbopt.backend import twoaddr
from qbopt.backend import allocate
from qbopt.backend import coalesce
from qbopt.backend import omfwrite
from qbopt.backend import peephole
from qbopt.backend import prologue
from qbopt.objectfile import module
from qbopt.optimize import transform
from qbopt.backend import frame as frames
from qbopt.frontend import blocks as split
from qbopt.frontend.blocks import code_map
from qbopt.model.passes import LIRTransform


def machine(
    pinned: dict, frame=None, calls: dict | None = None, *, basic_semantics: bool = False
) -> list[LIRTransform]:
    """Every phase between lowering and emission, in order."""
    from qbopt.backend import floatalloc

    if frame is not None and frame.native is not None:
        pinned = {**pinned, **frame.native_pins}
    return [
        floatalloc.FloatAlloc(frame, basic_semantics=basic_semantics),
        phielim.PhiElimination(),
        twoaddr.TwoAddress(),
        coalesce.Coalescer(),
        allocate.RegAlloc(pinned, frame),
        # After the allocation and before anything reads the code as a
        # sequence: which moves in a phi's copy conflict is a question
        # about locations, and until the allocator has chosen them there
        # is nothing to ask.
        parcopy.ParallelCopy(),
        prologue.Prologue(frame, calls) if frame is not None else prologue.Prologue(frames.Frame(0), calls),
        peephole.Peephole(frame),
    ]


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


def run(data: bytes, native_fpu: bool = False, optimise: bool = True) -> tuple[bytes, str]:
    """The object, rewritten, and what happened. The input back on refusal."""
    records = omf.parse(data)
    found = module.of(records)
    mapped = code_map(found)
    if isinstance(mapped, str):
        return data, mapped
    blocks = split.partition(found, mapped)

    # One map for the module, and the same object reaches the raise and
    # the lowering: a contract chosen twice can be chosen differently.
    contracts = runtime.for_module(found)
    raised = list(mir.bodies(found, blocks, contracts))
    if not raised:
        return data, "nothing to raise"

    from qbopt.optimize import rotate

    done = []
    for name, body in raised:
        if optimise:
            body = transform.applied(
                body, found.dgroup, found.calls, blocks=blocks, found=found, promote_=False, strength_=False
            )
            body = rotate.entered(body)
        low = verified(
            lower.lowered(name, body, found.calls, set(found.absorbed), contracts),
            "lower",
            in_ssa=True,
        )
        frame = frames.of(low, found.calls)
        in_ssa = True
        for phase in machine(_pinned(low), frame, found.calls):
            if isinstance(phase, phielim.PhiElimination):
                in_ssa = False
            low = checked(low, phase, in_ssa=in_ssa)
        done.append(low)

    reached = frozenset(at for block in blocks for insn in block.insns for at in range(insn.at, insn.end))
    fields = frozenset(one.offset for one in omf.fixups(records) if one.seg == found.seg)
    out = omfwrite.written_bc(found, done, records, {}, mapped.tables, fields, reached, native_fpu)
    return (data, out) if isinstance(out, str) else (out, "written")


def _pinned(body) -> dict:
    """A body's pins, by value id, which is what the allocator is keyed on.

    MIR pins a value; LIR names an id. The translation is here rather than
    in the allocator because a pin is the raise's statement about the
    machine, and this is the last place that holds both forms.
    """
    return {value.id: register for value, register in (getattr(body, "pins", None) or {}).items()}
