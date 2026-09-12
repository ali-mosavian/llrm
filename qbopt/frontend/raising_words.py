"""Normalize unobserved upper-word preservation at the raise boundary."""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model.mir import MirBody
from qbopt.analysis import liveness as alive_at


def carried(body: MirBody) -> MirBody:
    parents: dict[mir.Value, set[mir.Value]] = {}
    for block in body.blocks:
        for phi in block.phis:
            parents[phi.result] = set(phi.incoming.values())
        for op in block.ops:
            for source, result in op.merges.items():
                if mir.Held(result, 2) in op.results:
                    parents[result] = {source}
    sources = {value for incoming in parents.values() for value in incoming}
    roots = {value: set() if value in parents else {value} for value in sources | parents.keys()}
    while True:
        changed = False
        for value, incoming in parents.items():
            extended = set().union(*(roots[source] for source in incoming))
            if extended != roots[value]:
                roots[value] = extended
                changed = True
        if not changed:
            break
    canonical = {value: next(iter(found)) for value, found in roots.items() if len(found) == 1}

    def operation(op: mir.Op) -> mir.Op:
        # A merge carries only upper bits; ordinary operands must retain
        # their own values even when they also supply those bits.
        if not op.merges:
            return op
        merges = {canonical.get(source, source): result for source, result in op.merges.items()}
        if len(merges) != len(op.merges) or merges == op.merges:
            return op
        explicit = {arg.value for arg in op.args if isinstance(arg, mir.Held)}
        explicit.update(value for ref in op.loads + op.stores for value in (ref.base, ref.segment) if value is not None)
        uses = tuple(
            dict.fromkeys((*(value for value in op.uses if value not in op.merges or value in explicit), *merges))
        )
        return replace(op, merges=merges, uses=uses)

    return replace(
        body, blocks=tuple(replace(block, ops=tuple(operation(op) for op in block.ops)) for block in body.blocks)
    )


def scalar(body: MirBody) -> MirBody:
    required = leaving(body)
    carrying = []
    candidates = {}
    for block in body.blocks:
        carrying.extend((phi.result, value) for phi in block.phis for value in phi.incoming.values())
        for op in block.ops:
            carrying.extend((result, source) for source, result in op.merges.items())
            widths = {}
            for arg in op.args:
                if isinstance(arg, mir.Held):
                    widths[arg.value] = max(widths.get(arg.value, 0), arg.width)
            for ref in op.loads + op.stores:
                if ref.base is not None:
                    widths[ref.base] = max(widths.get(ref.base, 0), ref.base_width)
                if ref.segment is not None:
                    widths[ref.segment] = 4
            narrow = (
                op.kind in (mir.Kind.LOAD, mir.Kind.COPY)
                and not op.barrier
                and len(op.results) == 1
                and isinstance(op.results[0], mir.Held)
                and op.results[0].width == 2
                and all(value == op.results[0].value for value in op.merges.values())
            )
            if narrow:
                candidates[id(op)] = widths
            for value in op.uses:
                if value.flags:
                    continue
                if (
                    op.barrier
                    or op.kind is mir.Kind.OPAQUE
                    or widths.get(value, 0) >= 4
                    or (value not in widths and not (narrow and value in op.merges))
                ):
                    required.add(value)
    while True:
        extended = required | {source for result, source in carrying if result in required}
        if extended == required:
            break
        required = extended
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if id(op) in candidates:
                removed = {source for source, result in op.merges.items() if result not in required}
                if removed:
                    op = replace(
                        op,
                        merges={source: result for source, result in op.merges.items() if source not in removed},
                        uses=tuple(value for value in op.uses if value not in removed or value in candidates[id(op)]),
                    )
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return carried(replace(body, blocks=tuple(blocks)))


def leaving(body: MirBody) -> set:
    """The value each register holds where control leaves the body.

    What the caller reads is not a fact this body holds, so everything that
    reaches an exit counts as read. Reaching definitions forward, meeting by
    union: two definitions of one register arriving at a join are both still
    readable there, and claiming otherwise would kill a live one.

    This is the one place the liveness looks at `origin`. It has to: "what
    the caller sees" is a statement about registers, and there is nothing
    else in a MirBody that says which value ends up where.
    """
    preds = {block.at: [one.at for one in body.blocks if block.at in one.succ] for block in body.blocks}
    arriving: dict = {}
    for value in alive_at.entry_values(body):
        register = body.origin.get(value)
        if register is not None:
            arriving.setdefault(ir.ROOT.get(register, register), set()).add(value)

    outof: dict[int, dict] = {block.at: {} for block in body.blocks}
    changing = True
    while changing:
        changing = False
        for block in body.blocks:
            here: dict = {}
            coming = [outof[one] for one in preds[block.at]]
            if block.at == body.entry:
                coming.append(arriving)
            for one in coming:
                for register, values in one.items():
                    here.setdefault(register, set()).update(values)
            for phi in block.phis:
                # Flags the same way as below: what a caller reads is a
                # register, and the flags are not one of them. Skipped for
                # an operation's own defines and not for a phi's result,
                # the flag phi at a loop header reached the exit as though
                # it were a register value -- nothing overwrites that key,
                # since every operation skips it -- and was live with
                # nothing reading it. That kept the flags of every
                # operation feeding it alive too, and reuse refuses a
                # divide whose other answer is still wanted.
                if phi.result.flags:
                    continue
                register = body.origin.get(phi.result)
                if register is not None:
                    here[ir.ROOT.get(register, register)] = {phi.result}
            for op in block.ops:
                for value in op.defines:
                    if value.flags:
                        continue
                    register = body.origin.get(value)
                    if register is not None:
                        here[ir.ROOT.get(register, register)] = {value}
            if here != outof[block.at]:
                outof[block.at] = here
                changing = True

    out: set = set()
    for block in body.blocks:
        if block.succ:
            continue
        for values in outof[block.at].values():
            out |= values
    return out
