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
    write      LIR    -> bytes      objwrite.py

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

from qbopt import mir
from qbopt import omf
from qbopt import lower
from qbopt import module
from qbopt import parcopy
from qbopt import phielim
from qbopt import runtime
from qbopt import twoaddr
from qbopt import allocate
from qbopt import coalesce
from qbopt import objwrite
from qbopt import prologue
from qbopt import peephole
from qbopt import transform
from qbopt import blocks as split
from qbopt import frame as frames
from qbopt.blocks import code_map
from qbopt.passes import LIRTransform


def machine(pinned: dict, frame=None, calls: dict | None = None) -> list[LIRTransform]:
    """Every phase between lowering and emission, in order."""
    from qbopt import floatalloc
    return [
        floatalloc.FloatAlloc(),
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
        peephole.Peephole(),
    ]


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

    done = []
    for name, body in raised:
        if optimise:
            body = transform.applied(
                body, found.dgroup, found.calls, blocks=blocks, found=found, promote_=False, strength_=False
            )
            body = transform.widened(body)
        low = lower.lowered(name, body, found.calls, set(found.absorbed), contracts)
        frame = frames.of(low, found.calls)
        for phase in machine(_pinned(low), frame, found.calls):
            low = phase.transform(low)
        done.append(low)

    reached = frozenset(at for block in blocks for insn in block.insns for at in range(insn.at, insn.end))
    fields = frozenset(one.offset for one in omf.fixups(records) if one.seg == found.seg)
    out = objwrite.written(found, done, records, {}, mapped.tables, fields, reached, native_fpu)
    return (data, out) if isinstance(out, str) else (out, "written")


def _pinned(body) -> dict:
    """A body's pins, by value id, which is what the allocator is keyed on.

    MIR pins a value; LIR names an id. The translation is here rather than
    in the allocator because a pin is the raise's statement about the
    machine, and this is the last place that holds both forms.
    """
    return {value.id: register for value, register in (getattr(body, "pins", None) or {}).items()}
