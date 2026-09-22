"""Load narrowing: a wide load read only through its word halves is those words loaded.

LLVM's DAGCombiner does this for `trunc (load)` and `srl (load)`. It is
selection, not optimization: the MIR already says only the halves are read,
and a word is this target's natural load. Extracting the high word of a dword
register otherwise costs a push and two pops.
"""

from dataclasses import replace

from qbopt.model import mir

_WORD = 2
_HALVES = {0: 0, 16: _WORD}  # extract bit offset -> byte offset


def narrowed(body: mir.MirBody) -> mir.MirBody:
    """`body` with every narrowable dword load split into its read words."""
    readers: dict[mir.Value, list[mir.Op]] = {}
    for block in body.blocks:
        for op in block.ops:
            for value in op.uses:
                readers.setdefault(value, []).append(op)
    phis = {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    halves: dict[int, list[tuple[int, mir.Value]]] = {}
    gone: set[int] = set()
    for block in body.blocks:
        for op in block.ops:
            value = _loaded(op)
            if value is None or value in phis:
                continue
            reading = readers.get(value, [])
            offsets = [_half(one, value) for one in reading]
            if not reading or None in offsets:
                continue
            halves[id(op)] = [(offset, one.results[0].value) for offset, one in zip(offsets, reading)]
            gone.update(id(one) for one in reading)
    if not halves:
        return body
    return replace(
        body,
        blocks=tuple(
            replace(block, ops=tuple(one for op in block.ops for one in _rewritten(op, halves, gone)))
            for block in body.blocks
        ),
    )


def _loaded(op: mir.Op) -> "mir.Value | None":
    """The dword a plain load of one exactly addressed cell defines."""
    match op.kind, op.args, op.results:
        case mir.Kind.LOAD, (mir.Cell(ref=ref),), (mir.Held(value=value, width=4),):
            # A source-backed load's bytes belong to the source-map emitter.
            if (
                ref.width == 4
                and ref.addr is not None
                and not ref.volatile
                and op.loads == (ref,)
                and not op.source_backed
            ):
                return value
    return None


def _half(op: mir.Op, value: mir.Value) -> "int | None":
    """The byte offset of the word this operation extracts from `value`."""
    match op.kind, op.args, op.results:
        case mir.Kind.EXTRACT, (mir.Held(value=source), mir.Const(n=bit)), (mir.Held(width=2),):
            if source == value and bit in _HALVES and op.uses == (value,):
                return _HALVES[bit]
    return None


def _rewritten(op: mir.Op, halves: dict, gone: set[int]) -> tuple[mir.Op, ...]:
    if id(op) in gone:
        return ()
    if id(op) not in halves:
        return (op,)
    (ref,) = op.loads
    made: dict[int, mir.Value] = {}
    out = []
    for offset, result in sorted(halves[id(op)], key=lambda one: one[0]):
        if offset in made:
            out.append(
                replace(
                    op,
                    kind=mir.Kind.COPY,
                    name="",
                    defines=(result,),
                    uses=(made[offset],),
                    args=(mir.Held(made[offset], _WORD),),
                    results=(mir.Held(result, _WORD),),
                    loads=(),
                )
            )
            continue
        made[offset] = result
        word = replace(ref, addr=ref.addr.plus(offset), width=_WORD)
        out.append(
            replace(
                op,
                defines=(result,),
                args=(mir.Cell(word),),
                results=(mir.Held(result, _WORD),),
                loads=(word,),
            )
        )
    return tuple(out)
