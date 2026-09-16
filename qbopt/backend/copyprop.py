"""Remove physical register copies whose byte values agree on every CFG path."""

from dataclasses import replace
from itertools import combinations

from iced_x86 import Register

from qbopt.analysis import loops
from qbopt.model import ir, lir


def forwarded(body: lir.LirBody) -> lir.LirBody:
    from qbopt.backend.peephole import _lanes, _register_effects

    if any(block.phis for block in body.blocks):
        return body
    lanes = sorted(
        {
            lane
            for register in (
                Register.EAX,
                Register.EBX,
                Register.ECX,
                Register.EDX,
                Register.ESI,
                Register.EDI,
                Register.EBP,
            )
            for lane in _lanes(register)
        }
    )
    universe = frozenset(combinations(lanes, 2))
    recipes = {}
    for one in body.insns:
        what = one.what
        if (
            what is not None
            and what.op in (ir.Operation.BRANCH, ir.Operation.JUMP)
            and isinstance(what.target, int)
            and not what.sources
            and not what.dests
            and not one.clobbers
            and not one.requires
            and not one.delivers
        ):
            recipes[id(one)] = ((), ())
            continue
        effects = _register_effects(one, may_write=True)
        if effects is None:
            recipes[id(one)] = None
            continue
        copies = ()
        match what:
            case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as dest,), (ir.Reg() as source,)):
                destinations, sources = sorted(_lanes(dest.register)), sorted(_lanes(source.register))
                if len(destinations) == dest.width == source.width == len(sources):
                    copies = tuple(zip(destinations, sources))
        recipes[id(one)] = (effects[1], copies)

    def equal(facts, left, right):
        return left == right or tuple(sorted((left, right))) in facts

    def after(facts, one):
        recipe = recipes[id(one)]
        if recipe is None:
            return frozenset()
        writes, copies = recipe
        if copies:
            sources = dict(copies)
            return frozenset(
                (left, right)
                for left, right in universe
                if equal(facts, sources.get(left, left), sources.get(right, right))
            )
        return frozenset(pair for pair in facts if not writes.intersection(pair)) if writes else facts

    blocks = {block.at: block for block in body.blocks}
    reachable, pending = set(), [body.entry]
    while pending:
        at = pending.pop()
        if at in reachable or at not in blocks:
            continue
        reachable.add(at)
        pending.extend(blocks[at].succ)
    predecessors = loops.predecessors(body.blocks)
    # Start at the must-analysis top, then intersect paths to a fixed point.
    # Entry contributes no equality, so a backedge cannot invent its own proof.
    entries = {at: universe for at in reachable}
    exits = dict(entries)
    changed = True
    while changed:
        changed = False
        for block in body.blocks:
            if block.at not in reachable:
                continue
            parents = predecessors[block.at] & reachable
            incoming = (
                frozenset.intersection(*(exits[at] for at in parents))
                if parents and block.at != body.entry
                else frozenset()
            )
            entries[block.at] = incoming
            facts = incoming
            for one in block.insns:
                facts = after(facts, one)
            if exits[block.at] != facts:
                exits[block.at] = facts
                changed = True

    result = []
    for block in body.blocks:
        facts, redundant = entries.get(block.at, frozenset()), set()
        for one in block.insns:
            recipe = recipes[id(one)]
            if (
                recipe is not None
                and recipe[1]
                and block.at in reachable
                and not one.requires
                and not one.delivers
                and not one.spread
                and one.group is None
                and one.symbol is not True
                and all(equal(facts, left, right) for left, right in recipe[1])
            ):
                redundant.add(id(one))
            facts = after(facts, one)
        result.append(
            replace(
                block,
                insns=tuple(lir.anchor(one) if id(one) in redundant else one for one in block.insns),
            )
        )
    return replace(body, blocks=tuple(result))
