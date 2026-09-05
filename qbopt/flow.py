"""The whole flow, one function, in the order the architecture gives.

    parse -> raise -> passes -> lower -> allocate -> write

Six steps and five forms: bytes, MIR, MIR, LIR, LIR, bytes. Each step takes
one form and returns the next, and no step reaches past the one below it.
This exists so that reading the pipeline does not mean reading wholeseg.py,
which grew the same sequence inside one function with the last three steps
tangled together -- layout colouring, layout running a MIR pass, and the
allocation decided in the middle of emission.

Not yet the shipped path. rewrite.py still calls wholeseg.rebuilt(), and
what this one produces is measured against that rather than trusted over
it. The point of having it is that the seams are where the architecture
says they are and can be moved one at a time.
"""

from qbopt import allocate
from qbopt import blocks as split
from qbopt import layout
from qbopt import lower
from qbopt import mir
from qbopt import module
from qbopt import objwrite
from qbopt import omf
from qbopt import transform
from qbopt.blocks import code_map


def run(data: bytes, native_fpu: bool = False, optimise: bool = True) -> tuple[bytes, str]:
    """The object, rewritten, and what happened. The input back on refusal."""
    records = omf.parse(data)
    found = module.of(records)
    mapped = code_map(found)
    if isinstance(mapped, str):
        return data, mapped
    blocks = split.partition(found, mapped)

    raised = list(mir.bodies(found, blocks))
    if not raised:
        return data, "nothing to raise"

    done = []
    placed: dict = {}
    for name, body in raised:
        if optimise:
            body = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
            body = transform.widened(body)
        low = lower.lowered(name, body)
        done.append(allocate.applied(low, allocate.allocate(low, _pinned(body))))

    reached = frozenset(at for block in blocks for insn in block.insns for at in range(insn.at, insn.end))
    fields = frozenset(one.offset for one in omf.fixups(records) if one.seg == found.seg)
    out = objwrite.written(found, done, records, placed, mapped.tables, fields, reached, native_fpu)
    return (data, out) if isinstance(out, str) else (out, "written")


def _pinned(body) -> dict:
    """A body's pins, by value id, which is what the allocator is keyed on.

    MIR pins a value; LIR names an id. The translation is here rather than
    in the allocator because a pin is the raise's statement about the
    machine and this is the last place that still holds both forms.
    """
    return {value.id: register for value, register in (getattr(body, "pins", None) or {}).items()}
