"""How large a completely peeled loop will be, before anything is cloned.

GCC's `tree_estimate_loop_size` and `estimated_unrolled_size`
(tree-ssa-loop-ivcanon.cc): an operation whose operands are all constant once
the iteration is fixed -- constants, counters with a constant start and step,
and what those compute -- folds away in every copy; the rest is copied once
per iteration. Unroll and peel both ask this first. Building and optimizing a
candidate only to reject it cost modern nbody 14 times its compile time.
"""

from qbopt.model import mir
from qbopt.analysis import induction
from qbopt.model.passes import Where

_OPAQUE = frozenset({mir.Kind.CALL, mir.Kind.OPAQUE, mir.Kind.ESCAPE, mir.Kind.ARG, mir.Kind.RESULT})


def admitted(body: mir.MirBody, loop, count: int, facts: dict, where: Where) -> bool:
    """Whether a `count`-fold copy of `loop` can pass the target's peel budget."""
    size, folded = _sizes(body, loop, facts)
    copied = count * (size - folded)
    if copied <= size:
        return True
    if not where.options.grows:
        return False
    limits = where.options
    if limits.max_unroll_iterations and count > limits.max_unroll_iterations:
        return False
    # GCC credits a third of what is left as likely to fold after all.
    return not limits.max_unrolled_operations or copied - copied // 3 <= limits.max_unrolled_operations


def _sizes(body: mir.MirBody, loop, facts: dict) -> tuple[int, int]:
    """The loop's operations, and how many of them fold once the iteration is fixed."""
    inside = [block for block in body.blocks if block.at in loop.body]
    known = {
        one.value
        for one in induction.basics(body, loop).values()
        if _constant(one.start, facts) and _constant(one.step, facts)
    }
    known |= {value.id for value, fact in facts.items() if fact is not None}
    ops = [op for block in inside for op in block.ops if op.kind is not mir.Kind.NOTHING]
    folded: set[int] = set()
    changed = True
    while changed:
        changed = False
        for op in ops:
            if id(op) in folded or not _pure(op):
                continue
            if all(value.id in known for value in op.uses):
                folded.add(id(op))
                known.update(value.id for value in op.defines)
                changed = True
    phis = sum(len(block.phis) for block in inside)
    return len(ops) + phis, len(folded) + sum(phi.result.id in known for block in inside for phi in block.phis)


def _constant(arg: mir.Arg, facts: dict) -> bool:
    return isinstance(arg, mir.Const) or isinstance(arg, mir.Held) and facts.get(arg.value) is not None


def _pure(op: mir.Op) -> bool:
    return not (op.loads or op.stores or op.barrier or op.kind in _OPAQUE)


def signature(body: mir.MirBody, loop, count: int, facts: dict) -> tuple:
    """The loop as a candidate sees it, with incidental value numbering removed.

    Two rounds of the fixed point renumber every value; the same loop, with
    the same constants reaching it, is the same candidate and gets the same
    answer. A constant newly reaching it -- after its outer loop is peeled --
    makes it a different one.
    """
    names: dict[int, int] = {}

    def value(one: mir.Value) -> tuple:
        names.setdefault(one.id, len(names))
        fact = facts.get(one)
        return names[one.id], None if fact is None else (fact.n, fact.width)

    def arg(one: mir.Arg) -> object:
        match one:
            case mir.Held():
                return value(one.value), one.width
            case mir.Cell(ref=ref):
                reached = tuple(value(part) if part is not None else None for part in (ref.base, ref.segment))
                return repr(ref.addr), ref.width, reached
        return repr(one)

    parts: list = [count]
    for block in body.blocks:
        if block.at not in loop.body:
            continue
        parts.append(
            tuple((value(phi.result), tuple(sorted(value(one) for one in phi.incoming.values()))) for phi in block.phis)
        )
        parts.extend(
            (op.kind, tuple(map(arg, op.args)), tuple(map(arg, op.results)))
            for op in block.ops
            if op.kind is not mir.Kind.NOTHING
        )
    return tuple(parts)
