"""Expose the independently consumed words of packed far-pointer accesses.

A packed selector:offset value is a scalar program value.  A dereference is
different: its offset and selector are two address operands with independent
lifetimes.  Keeping the packed scalar as the memory operand hides those
lifetimes from CSE, LICM and allocation, forcing lowering to unpack it locally
through the stack for every access.

This normalization is deliberately machine-neutral.  It introduces ordinary
MIR bit extractions and a far-space reference whose base and selector are SSA
values.  Lowering still chooses how those values reach the target address.
"""

from dataclasses import replace

from qbopt.analysis import ssa
from qbopt.model import mir
from qbopt.objectfile.module import Addr, Space


def _packed(ref: mir.MemRef) -> bool:
    """Whether ``ref`` is the canonical supported packed dereference."""
    return ref.pointer and ref.base is not None and ref.base_width == 4 and ref.addr is None and ref.segment is None


def _reference(ref: mir.MemRef, pieces: dict[mir.Value, tuple[mir.Value, mir.Value]]) -> mir.MemRef:
    """One packed reference with its already-named address components."""
    if not _packed(ref):
        return ref
    assert ref.base is not None
    low, high = pieces[ref.base]
    return replace(
        ref,
        addr=Addr(Space.FAR, 0),
        base=low,
        segment=high,
        space=Space.FAR,
        base_width=2,
        pointer=False,
    )


def split(body: mir.MirBody) -> mir.MirBody:
    """Name each packed access's offset and selector as word SSA values."""
    if not any(_packed(ref) for block in body.blocks for op in block.ops for ref in (*op.loads, *op.stores)):
        return body

    values = tuple(ssa.values(body))
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)

    def fresh(at: int) -> mir.Value:
        nonlocal serial, variable
        serial += 1
        variable += 1
        return mir.Value(serial, at, variable=variable, version=1)

    def extract(source: mir.Value, result: mir.Value, offset: int, at: int) -> mir.Op:
        return mir.Op(
            at,
            mir.Synth.HALF_TO_LOW,
            "extract",
            (result,),
            (source,),
            kind=mir.Kind.EXTRACT,
            args=(mir.Held(source, 4), mir.Const(offset, 4)),
            results=(mir.Held(result, 2),),
        )

    changed = False
    blocks = []
    for block in body.blocks:
        # Reuse within a block immediately.  Dominating instances in other
        # blocks are ordinary redundant expressions for GVN to eliminate.
        pieces: dict[mir.Value, tuple[mir.Value, mir.Value]] = {}
        operations = []
        for op in block.ops:
            references = tuple(
                dict.fromkeys(
                    ref
                    for ref in (
                        *(arg.ref for arg in op.args if isinstance(arg, mir.Cell)),
                        *(result.ref for result in op.results if isinstance(result, mir.Cell)),
                        *op.loads,
                        *op.stores,
                        *(ref for ref, _known in op.memory_values),
                    )
                    if _packed(ref)
                )
            )
            if not references:
                operations.append(op)
                continue

            made = []
            for ref in references:
                assert ref.base is not None
                if ref.base in pieces:
                    continue
                low, high = fresh(op.at), fresh(op.at)
                pieces[ref.base] = (low, high)
                made.extend((extract(ref.base, low, 0, op.at), extract(ref.base, high, 16, op.at)))

            direct = {arg.value for arg in op.args if isinstance(arg, mir.Held)} | set(op.exits) | set(op.merges)
            packed_values = {ref.base for ref in references if ref.base is not None}
            uses = []
            for value in op.uses:
                if value not in packed_values or value in direct:
                    uses.append(value)
            for value in packed_values:
                uses.extend(pieces[value])

            operations.extend(made)
            operations.append(
                replace(
                    op,
                    uses=tuple(dict.fromkeys(uses)),
                    args=tuple(
                        mir.Cell(_reference(arg.ref, pieces)) if isinstance(arg, mir.Cell) else arg for arg in op.args
                    ),
                    results=tuple(
                        mir.Cell(_reference(result.ref, pieces)) if isinstance(result, mir.Cell) else result
                        for result in op.results
                    ),
                    loads=tuple(_reference(ref, pieces) for ref in op.loads),
                    stores=tuple(_reference(ref, pieces) for ref in op.stores),
                    memory_values=tuple((_reference(ref, pieces), known) for ref, known in op.memory_values),
                )
            )
            changed = True
        blocks.append(replace(block, ops=tuple(operations)))

    return replace(body, blocks=tuple(blocks)) if changed else body
