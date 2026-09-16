"""Forward and remove physical register copies proven on every CFG path.

LLVM's MachineCopyPropagation tracks physical register units after allocation;
GCC's ``regcprop`` chooses an older equivalent hard register and validates the
changed instruction.  Here a unit is one byte lane.  Undirected lane equality
removes redundant copies, while a second must-analysis retains the reaching
copy's direction for operand substitution.  Selection and decoded effects are
both checked again before a source is renamed.
"""

from dataclasses import replace
from itertools import combinations

from iced_x86 import Register
from iced_x86 import Register_

from qbopt.model import ir
from qbopt.model import lir
from qbopt.analysis import loops

Lane = tuple[Register_, int]
Relations = frozenset[tuple[Lane, Lane]]


def forwarded(body: lir.LirBody) -> lir.LirBody:
    from qbopt.backend import select
    from qbopt.backend import target
    from qbopt.backend.peephole import _lanes
    from qbopt.backend.peephole import _register_effects

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
    register_for = {
        tuple(sorted(_lanes(register))): register
        for register in target.WIDTHS
        if _lanes(register) and Register.ESP not in {ir.root(register)}
    }
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
                    copies = tuple(zip(destinations, sources, strict=True))
        recipes[id(one)] = (effects[1], copies)

    def equal(facts: Relations, left: Lane, right: Lane) -> bool:
        return left == right or tuple(sorted((left, right))) in facts

    def after(facts: Relations, one: lir.Insn) -> Relations:
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

    def directed_after(directed: Relations, one: lir.Insn) -> Relations:
        """Available copy sources, invalidated like LLVM's register units.

        Equality is undirected, but forwarding has a useful direction: for
        ``bx = ax``, AX is the older value and using it can make the copy
        dead.  Keep that direction only while both byte lanes survive.  At a
        join, the ordinary dataflow intersection below requires every path to
        name the same source.
        """
        recipe = recipes[id(one)]
        if recipe is None:
            return frozenset()
        writes, copies = recipe
        before = dict(directed)

        def oldest(lane: Lane) -> Lane:
            seen: set[Lane] = set()
            while lane in before and lane not in seen:
                seen.add(lane)
                lane = before[lane]
            return lane

        # Resolve sources before killing the destination: a reverse copy such
        # as AX=BX; BX=AX still reads AX's old value even though BX is written.
        sources = {dest: oldest(source) for dest, source in copies}
        out = {dest: source for dest, source in before.items() if dest not in writes and source not in writes}
        for dest, source in copies:
            origin = sources[dest]
            if origin in writes:
                origin = source
            if dest != origin:
                out[dest] = origin
        return frozenset(out.items())

    def forward_use(one: lir.Insn, directed: Relations, facts: Relations) -> lir.Insn:
        """Use the reaching copy's source in explicit, independently encoded operands."""
        if (
            one.what is None
            or one.clobbers
            or one.clobbers_high
            or one.requires
            or one.delivers
            or one.spread
            or one.group is not None
            or one.symbol is True
        ):
            return one
        mapping = dict(directed)
        changed = one.what
        for index, source in enumerate(changed.sources):
            if not isinstance(source, ir.Reg):
                continue
            source_lanes = tuple(sorted(_lanes(source.register)))
            candidate_lanes = tuple(mapping.get(lane, lane) for lane in source_lanes)
            candidate = register_for.get(tuple(sorted(candidate_lanes)))
            if candidate is None or candidate == source.register:
                continue
            replacement = ir.Reg(candidate, source.width)
            if not all(equal(facts, left, right) for left, right in zip(source_lanes, candidate_lanes, strict=True)):
                continue
            before_effects = _register_effects(replace(one, what=changed), may_write=True)
            if before_effects is None or set(candidate_lanes) & before_effects[1]:
                continue
            sources = list(changed.sources)
            sources[index] = replacement
            proposed = replace(changed, sources=tuple(sources))
            if select.emit(proposed) is None:
                continue
            after_effects = _register_effects(replace(one, what=proposed), may_write=True)
            expected_reads = (before_effects[0] - set(source_lanes)) | set(candidate_lanes)
            if after_effects is None or after_effects[1] != before_effects[1] or after_effects[0] != expected_reads:
                continue
            changed = proposed
        return one if changed == one.what else replace(one, what=changed)

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
    directed_entries = {at: frozenset() for at in reachable}
    directed_exits = dict(directed_entries)
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
            directed_incoming = (
                frozenset.intersection(*(directed_exits[at] for at in parents))
                if parents and block.at != body.entry
                else frozenset()
            )
            directed_entries[block.at] = directed_incoming
            facts = incoming
            directed = directed_incoming
            for one in block.insns:
                facts = after(facts, one)
                directed = directed_after(directed, one)
            if exits[block.at] != facts or directed_exits[block.at] != directed:
                exits[block.at] = facts
                directed_exits[block.at] = directed
                changed = True

    result = []
    for block in body.blocks:
        facts, redundant = entries.get(block.at, frozenset()), set()
        directed = directed_entries.get(block.at, frozenset())
        rewritten = []
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
            rewritten.append(forward_use(one, directed, facts))
            facts = after(facts, one)
            directed = directed_after(directed, one)
        result.append(
            replace(
                block,
                insns=tuple(
                    lir.anchor(original) if id(original) in redundant else replacement_insn
                    for original, replacement_insn in zip(block.insns, rewritten, strict=True)
                ),
            )
        )
    return replace(body, blocks=tuple(result))
