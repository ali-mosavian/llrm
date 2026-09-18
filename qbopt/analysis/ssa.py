from dataclasses import replace
from collections.abc import Iterator

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model import memory
from qbopt.model.mir import Op
from qbopt.model.mir import MirBody


def pruned_phis(body: MirBody, roots: set[mir.Value]) -> MirBody:
    needed = roots | set(mir.exposed(body))
    needed |= {value for block in body.blocks for op in block.ops for value in mir.consumed(op)}
    needed |= {
        value
        for block in body.blocks
        for op in block.ops
        for ref in (*op.loads, *op.stores)
        for value in (ref.base, ref.segment)
        if value is not None
    }
    phis = {phi.result: phi for block in body.blocks for phi in block.phis}
    pending = list(needed & phis.keys())
    while pending:
        phi = phis[pending.pop()]
        incoming = set(phi.incoming.values()) - needed
        needed.update(incoming)
        pending.extend(incoming & phis.keys())
    removed = phis.keys() - needed
    if not removed:
        return body
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                phis=tuple(phi for phi in block.phis if phi.result not in removed),
                ops=tuple(
                    replace(
                        op,
                        uses=tuple(value for value in op.uses if value not in removed),
                        merges={source: target for source, target in op.merges.items() if source not in removed},
                    )
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )


def provider(value: mir.Value, swap: dict[int, mir.Value]) -> mir.Value:
    seen = set()
    while value.id in swap and swap[value.id] != value:
        if value.id in seen:
            raise ValueError("cyclic value substitution")
        seen.add(value.id)
        value = swap[value.id]
    return value


def substituted(op: Op, swap: dict[int, mir.Value]) -> Op:
    if not swap:
        return op

    def value(one: mir.Value | None) -> mir.Value | None:
        return provider(one, swap) if one is not None else None

    def reference(ref: mir.MemRef) -> mir.MemRef:
        return replace(ref, base=value(ref.base), segment=value(ref.segment))

    def operand(one: mir.Arg) -> mir.Arg:
        match one:
            case mir.Held(value=held, width=width):
                return mir.Held(provider(held, swap), width)
            case mir.Cell(ref=ref):
                return mir.Cell(reference(ref))
            case _:
                return one

    return replace(
        op,
        uses=tuple(provider(one, swap) for one in op.uses),
        exits=tuple(provider(one, swap) for one in op.exits),
        args=tuple(operand(one) for one in op.args),
        results=tuple(operand(one) if isinstance(one, mir.Cell) else one for one in op.results),
        loads=tuple(reference(ref) for ref in op.loads),
        stores=tuple(reference(ref) for ref in op.stores),
        memory_values=tuple((reference(ref), known) for ref, known in op.memory_values),
        merges={provider(source, swap): mask for source, mask in op.merges.items()},
    )


def cloned_pointer_metadata(
    body: MirBody,
    mappings: Iterator[dict[int, mir.Value]],
) -> tuple[frozenset[mir.Value], dict[mir.Value, "memory.Provenance"]]:
    """Carry semantic pointer facts onto fresh SSA definitions.

    Structural transformations clone values by id.  Pointer classification
    and frontend-established object roots are properties of those values,
    not of their old numeric ids, so every clone needs the corresponding
    side-table entry before alias analysis runs again.
    """
    pointer_ids = {value.id for value in body.pointer_values}
    seeds = {value.id: provenance for value, provenance in body.pointer_seeds.items()}
    pointer_values = set(body.pointer_values)
    pointer_seeds = dict(body.pointer_seeds)
    for mapping in mappings:
        for original, cloned in mapping.items():
            if original in pointer_ids:
                pointer_values.add(cloned)
            if original in seeds:
                pointer_seeds[cloned] = seeds[original]
    return frozenset(pointer_values), pointer_seeds


def renumbered(body: MirBody, variable: int) -> MirBody:
    """Make one variable's versions run from one without a gap.

    Construction numbers the definitions it is shown, so a definition removed
    afterwards leaves a hole -- and versions 2,3,4,5 read as a different
    variable from the one written four times, which is the whole point of a
    variable having versions rather than four names.
    """
    order = sorted({one for one in values(body) if one.variable == variable}, key=lambda one: one.version)
    swap = {one.id: replace(one, version=index) for index, one in enumerate(order, 1)}
    if all(one.version == swap[one.id].version for one in order):
        return body

    def named(one: "mir.Value | None") -> "mir.Value | None":
        return swap.get(one.id, one) if one is not None else None

    def reference(ref: mir.MemRef) -> mir.MemRef:
        return replace(ref, base=named(ref.base), segment=named(ref.segment))

    def operand(one: mir.Arg) -> mir.Arg:
        match one:
            case mir.Held(value=held, width=width):
                return mir.Held(named(held), width)
            case mir.Cell(ref=ref):
                return mir.Cell(reference(ref))
            case _:
                return one

    blocks = []
    for block in body.blocks:
        phis = tuple(
            replace(phi, result=named(phi.result), incoming={at: named(one) for at, one in phi.incoming.items()})
            for phi in block.phis
        )
        ops = tuple(
            replace(
                op,
                defines=tuple(named(one) for one in op.defines),
                uses=tuple(named(one) for one in op.uses),
                exits=tuple(named(one) for one in op.exits),
                args=tuple(operand(one) for one in op.args),
                results=tuple(operand(one) for one in op.results),
                loads=tuple(reference(one) for one in op.loads),
                stores=tuple(reference(one) for one in op.stores),
                merges={named(source): mask for source, mask in op.merges.items()},
            )
            for op in block.ops
        )
        blocks.append(replace(block, phis=phis, ops=ops))
    return replace(body, blocks=tuple(blocks))


def constructed(body: MirBody, variables: frozenset[int]) -> MirBody:
    def owned(values: tuple[mir.Value, ...]) -> tuple[mir.Value, ...]:
        return tuple(one for one in values if one.variable in variables)

    edges: dict[int, dict[int, mir.Value]] = {}
    for block in body.blocks:
        for phi in block.phis:
            for predecessor, value in phi.incoming.items():
                if value.variable in variables:
                    edges.setdefault(predecessor, {})[value.variable] = value

    # Phi inputs are reads at the predecessor's end, not at the merge block.
    probes = {at: (Op(at, ir.Operation.MOVE, "", (), tuple(names.values())),) for at, names in edges.items()}

    skeleton = replace(
        body,
        blocks=tuple(
            replace(
                block,
                phis=(),
                ops=tuple(
                    replace(
                        op,
                        defines=owned(op.defines),
                        uses=owned(op.uses),
                        exits=owned(op.exits),
                        args=(),
                        results=(),
                        loads=(),
                        stores=(),
                        merges={},
                        raised=None,
                    )
                    for op in block.ops
                )
                + probes.get(block.at, ()),
            )
            for block in body.blocks
        ),
    )
    repaired = mir.resolved(skeleton)
    if isinstance(repaired, str):
        raise mir.Unraisable(repaired)

    def merge(op: Op, fixed: Op) -> Op:
        uses = {one.variable: one for one in fixed.uses}
        defines = {one.variable: one for one in fixed.defines}

        def operand(one: mir.Arg, names: dict[int, mir.Value]) -> mir.Arg:
            return (
                mir.Held(names[one.value.variable], one.width)
                if (isinstance(one, mir.Held) and one.value.variable in names)
                else one
            )

        changed = substituted(op, {one.id: uses[one.variable] for one in op.uses if one.variable in uses})
        return replace(
            changed,
            uses=tuple(uses.get(one.variable, one) for one in op.uses),
            defines=tuple(defines.get(one.variable, one) for one in op.defines),
            results=tuple(operand(one, defines) for one in changed.results),
        )

    # The isolated renamer starts ids at zero; keep its namespace disjoint.
    offset = max((one.id for one in values(body)), default=0) + 1

    def shifted(value: mir.Value) -> mir.Value:
        return replace(value, id=value.id + offset)

    mapping = {one: shifted(one) for one in values(repaired)}
    repaired = replace(
        repaired,
        blocks=tuple(
            replace(
                block,
                phis=tuple(
                    mir.Phi(mapping[phi.result], {at: mapping[value] for at, value in phi.incoming.items()})
                    for phi in block.phis
                ),
                ops=tuple(
                    replace(
                        op,
                        uses=tuple(mapping[one] for one in op.uses),
                        exits=tuple(mapping[one] for one in op.exits),
                        defines=tuple(mapping[one] for one in op.defines),
                    )
                    for op in block.ops
                ),
            )
            for block in repaired.blocks
        ),
    )
    outgoing = {
        block.at: {value.variable: value for value in block.ops[-1].uses}
        for block in repaired.blocks
        if block.at in probes
    }
    repaired_by_at = {block.at: block for block in repaired.blocks}
    return replace(
        body,
        blocks=tuple(
            block
            if fixed is None
            else replace(
                block,
                phis=tuple(
                    replace(
                        phi,
                        incoming={
                            at: outgoing.get(at, {}).get(value.variable, value) for at, value in phi.incoming.items()
                        },
                    )
                    for phi in block.phis
                )
                + fixed.phis,
                ops=tuple(
                    merge(op, changed) for op, changed in zip(block.ops, fixed.ops[: len(block.ops)], strict=True)
                ),
            )
            for block in body.blocks
            for fixed in (repaired_by_at.get(block.at),)
        ),
    )


def values(body: MirBody) -> Iterator[mir.Value]:
    for block in body.blocks:
        for op in block.ops:
            yield from op.defines
            yield from op.uses
            yield from op.exits
        for phi in block.phis:
            yield phi.result
            yield from phi.incoming.values()
