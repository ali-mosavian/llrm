"""
Which register each value lives in.

BC allocates nothing: it keeps values in memory and reaches for a register
only long enough to compute with one. Everything measured about this pass's
remaining opportunity comes back to that -- of 384 forwardable loads, 308
need the value to end up in a different register from the one BC wrote it
to, which is not a rewrite anything could emit until something decides where
values live.

SSA is what makes the deciding tractable. Interference in an SSA program is
a chordal graph, so a greedy colouring in dominance order is optimal -- no
iterated coalescing, no build-simplify-select loop. The whole allocator is
liveness, then one walk down the dominator tree.

liveness.Liveness here is the standard formulation and the phi handling is the part
worth stating, because it is where a plausible-looking version goes wrong:

    live_out(B) = union over successors S of
                      live_in(S)  +  {phi.incoming[B] for phi in phis(S)}
    live_in(B)  = (live_out(B) - defs(B)) + upward-exposed uses(B)

A phi argument is live out of the *predecessor it comes from*, not live in
to the block holding the phi -- it is a value on an edge. Attributing it to
the phi's own block instead makes every argument of every phi live at the
join simultaneously, which reports pressure the program never had. A first
attempt at this measured a peak of twelve live values in a body BC compiled
into six registers, which is impossible: renaming does not change what is
live, only what it is called.
"""

from dataclasses import replace

from iced_x86 import Register_

from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.backend import target
from qbopt.model.mir import NAMES
from qbopt.model.mir import Value
from qbopt.analysis import liveness

# The register file is target.py's. What is left here is the allocation
# itself. The flags are not a register anything can be put in, and
# mir.FLAGS values are excluded from pressure and colouring alike.


def _addressing(body: mir.MirBody) -> set[Value]:
    """Every value some instruction requires in an addressing register.

    target.reads() says which registers an operation needs and how tightly.
    This asks it rather than re-deriving the answer from the operands, so
    there is one statement of what the machine requires and not three.
    """
    found: set[Value] = set()
    for block in body.blocks:
        for op in block.ops:
            what = lower.current(op, node=getattr(op, "node", None))
            if what is None:
                continue
            wanted = {where for where, need in target.reads(what).items() if need.fixed is None}
            found |= {value for value in op.uses if ir.ROOT.get(body.origin.get(value, -1), -1) in wanted}
    return found


def required(body: mir.MirBody) -> dict[Value, Register_]:
    """Every value an instruction requires in one particular register.

    The companion to _addressing, which asks the same table the other
    question: not "which registers may this operand live in" but "which one
    must it". A widening `imul` multiplies by ax and names it nowhere,
    `idiv` reads dx:ax, `cwd` and `cdq` extend ax, and a shift by a variable
    amount takes it in cl. lir.py is where all of that is written down.

    Nothing consulted it. That was safe only by accident: the identity
    assignment puts every value back where BC had it, so every requirement
    was already satisfied and none was ever tested. It stops being safe the
    moment a value moves, which is what hoisting a run does -- and the hoist
    refuses any run holding one of these operations for exactly that reason,
    having once renamed a multiplicand to cx and left the multiply reading
    ax. It printed 0 for 630.
    """
    out: dict[Value, Register_] = {}
    for block in body.blocks:
        for op in block.ops:
            what = lower.current(op, node=getattr(op, "node", None))
            if what is None:
                continue
            for side, needs in ((op.uses, target.reads(what)), (op.defines, target.writes(what))):
                for where, need in needs.items():
                    if need.fixed is None:
                        continue
                    for value in side:
                        if ir.ROOT.get(body.origin.get(value, -1), -1) is where:
                            out[value] = need.fixed
    return out


# Where it belongs: it names no register, and liveness.py is already where
# the analysis it is one line of lives.
pressure = liveness.pressure


def clobbered(body: mir.MirBody, found: liveness.Liveness | None = None) -> dict[Value, frozenset]:
    """Which registers each value may not sit in, because it lives across
    an operation that destroys them.

    An absorbed divide is a sequence, not an instruction: it writes the
    dividend's register, the divisor's, `idiv`'s own edx and wherever the
    answer it was not asked for is kept, and its operands name none of
    them. `lower._clobbers` has said so all along and nothing here read
    it -- safe only while every value stayed where BC had it, since BC's
    own call clobbered the same registers. It stops being safe the moment
    a value moves, which is what hoisting a divide does.

    Live *across*, not merely live: a value the operation itself defines,
    or one whose last read it is, is not crossing it.
    """
    found = found or liveness.live(body)
    out: dict[Value, set] = {}
    for block in body.blocks:
        after = set(found.live_out[block.at])
        for op in reversed(block.ops):
            # Not a use that is only the previous contents of what the op
            # writes. The restore idiom names the register its own high
            # half lands in, and counting that as a value crossing the
            # divide in front of it moved lngmix's accumulator out of dx
            # for nothing -- 1518 cycles to 1616, same bytes.
            before = (after - set(op.defines)) | {one for one in op.uses if one not in op.merges}
            takes = lower.clobbering(op)
            if takes:
                for one in (before & after) - set(op.defines):
                    if not one.flags:
                        out.setdefault(one, set()).update(takes)
            after = before
    return {one: frozenset(where) for one, where in out.items()}


def interference(body: mir.MirBody, found: liveness.Liveness | None = None) -> dict[Value, frozenset[Value]]:
    """Which values are ever live at the same moment.

    Built by walking each block backwards over its own live set rather than
    by comparing live ranges: two values interfere exactly when both are in
    that set at some point, which is the definition and needs no
    approximation. Flags are excluded -- they are not a register anything
    is placed in.
    """
    found = found or liveness.live(body)
    graph: dict[Value, set[Value]] = {}

    def meet(alive: set[Value]) -> None:
        real = [one for one in alive if not one.flags]
        for one in real:
            graph.setdefault(one, set()).update(other for other in real if other != one)

    for block in body.blocks:
        alive = set(found.live_out[block.at])
        meet(alive)
        for op in reversed(block.ops):
            alive -= set(op.defines)
            alive |= set(op.uses)
            meet(alive)
        for phi in block.phis:
            # Not a flags phi. meet() keeps flags out of the graph
            # everywhere else, and this line put them back: segld's latch
            # merges one, so f1 entered the graph here, asked for a
            # register in the greedy pass, found NONE was not one it could
            # use, and took eax from a value that wanted it.
            if not phi.result.flags:
                graph.setdefault(phi.result, set())
    return {one: frozenset(others) for one, others in graph.items()}


def congruent(body: mir.MirBody) -> dict[Value, Value]:
    """Each value mapped to the class a phi ties it into.

    A phi is not an instruction. Nothing runs on the edge to bring a value
    across, so a phi's result and every value arriving at it occupy one
    register or the program is wrong -- segld's inner counter was defined
    into dx and read back out of ax through the phi between them, and never
    reached its bound.

    So the unit an allocation moves is not a value, it is the whole class.
    BC's own code is congruent by construction, which is why the identity
    assignment has always been valid and why nothing needed this until
    something pinned.
    """
    parent: dict[Value, Value] = {}

    def root(one: Value) -> Value:
        parent.setdefault(one, one)
        while parent[one] is not one:
            parent[one] = parent[parent[one]]
            one = parent[one]
        return one

    for block in body.blocks:
        for phi in block.phis:
            if phi.result.flags:
                continue
            for value in phi.incoming.values():
                if value.flags:
                    continue
                here, there = root(phi.result), root(value)
                if here is not there:
                    parent[here] = there

        # And a two-address instruction, which reads and writes one
        # register: `add ax,[c]` is ax at two moments, not two places.
        # Allocating the halves apart emits `add di,[c]`, adding to
        # whatever di held -- matrix printed -2076 for 380. lir.tied says
        # which operations are one; x86 says it by naming the operand twice
        # and nothing else in the pipeline knew.
        for op in block.ops:
            what = lower.current(op, node=getattr(op, "node", None))
            if what is None:
                continue
            where = target.tied(what)
            if where is None:
                continue
            for one in op.defines:
                if one.flags or ir.ROOT.get(body.origin.get(one, -1), -1) is not where:
                    continue
                for other in op.uses:
                    if other.flags or ir.ROOT.get(body.origin.get(other, -1), -1) is not where:
                        continue
                    here, there = root(one), root(other)
                    if here is not there:
                        parent[here] = there
    return {one: root(one) for one in parent}


def _tangled(body: mir.MirBody) -> set:
    """Congruence classes two of whose members are live at once.

    The lost-copy shape: a value arriving at a phi is still wanted after
    it, so the phi's result and its own argument overlap and no single
    register holds both. BC's own code never has one -- measured, zero
    across the corpus's 579 raised bodies -- because it is congruent by
    construction. They appear when a pass moves a definition.
    """
    graph = interference(body)
    klass = congruent(body)
    of = {one: klass.get(one, one) for one in graph}
    return {of[one] for one, others in graph.items() for other in others if of[one] is of.get(other)}


def untangled(body: mir.MirBody) -> mir.MirBody:
    """A copy on the phi edge, where a class cannot otherwise be moved.

    colour() records a tangled class and refuses to move it, which is right
    -- nothing runs on the edge, so a phi's result and its arguments have to
    already share a register. The way out is to put something on the edge:
    `v' = v` at the end of the predecessor, with the phi taking v' instead.
    Then v' shares the phi's register and v is free to live elsewhere.

    This is the live range split, done where the split belongs. Without it a
    hoisted definition whose class is tangled cannot move at all, and
    hotlop's product stayed in ax for the counter to overwrite.
    """
    tangled = _tangled(body)
    if not tangled:
        return body
    graph = interference(body)
    klass = congruent(body)
    of = {one: klass.get(one, one) for one in graph}
    fresh = max((one.id for one in body.values), default=0) + 1

    added: dict[int, list] = {}
    swaps: dict[int, dict] = {}
    origin = dict(body.origin)
    for block in body.blocks:
        for phi in block.phis:
            if of.get(phi.result) not in tangled:
                continue
            where = origin.get(phi.result)
            if where is None:
                continue
            for came, value in phi.incoming.items():
                if not (graph.get(value, frozenset()) & {phi.result}):
                    continue
                copy = mir.Value(fresh, came)
                fresh += 1
                origin[copy] = where
                added.setdefault(came, []).append(
                    mir.Op(
                        at=came,
                        op=ir.Operation.MOVE,
                        kind=mir.Kind.COPY,
                        name="mov",
                        defines=(copy,),
                        uses=(value,),
                        # By value, not by register. Both sides have the
                        # same origin -- that is what the tangle is -- so
                        # naming registers here writes `mov dx,dx`, which
                        # lir.tied reads as two-address and congruent()
                        # ties straight back into the class this exists to
                        # break. MIR operands leave both to lowering and
                        # allocation without storing a selected instruction.
                            results=(mir.Held(copy, 2),),
                            args=(mir.Held(value, 2),),
                            raised=((), ()),
                        )
                )
                swaps.setdefault(id(phi), {})[came] = copy
    # And the other way a class tangles, which is not a phi at all. `add
    # ax,[c]` is `v1 = v0 + [c]` -- lir.tied says one register, congruent()
    # puts v0 and v1 in one class -- and where v0 is still wanted after the
    # operation the two are live at once. arridx is exactly {v22, v24} and
    # neither is a phi result.
    #
    # The copy goes before the operation rather than on an edge: `t = v0`,
    # and the operation reads t. Then t and v1 share a register and v0 is
    # free to live elsewhere, which is what a two-address machine needs
    # whenever the source outlives the instruction.
    ahead: dict[int, list] = {}
    reads: dict[int, dict] = {}
    for block in body.blocks:
        for op in block.ops:
            what = lower.current(op, node=getattr(op, "node", None))
            if what is None or target.tied(what) is None:
                continue
            for one in op.defines:
                if one.flags or of.get(one) not in tangled:
                    continue
                for other in op.uses:
                    if other.flags or of.get(other) is not of.get(one):
                        continue
                    if not (graph.get(other, frozenset()) & {one}):
                        continue
                    copy = mir.Value(fresh, op.at)
                    fresh += 1
                    origin[copy] = origin.get(other, origin.get(one))
                    ahead.setdefault(op.at, []).append(
                        mir.Op(
                            at=op.at,
                            op=ir.Operation.MOVE,
                            kind=mir.Kind.COPY,
                            name="mov",
                            defines=(copy,),
                            uses=(other,),
                            results=(mir.Held(copy, 2),),
                            args=(mir.Held(other, 2),),
                            raised=((), ()),
                        )
                    )
                    reads.setdefault(op.at, {})[other] = copy

    if not added and not ahead:
        return body

    out = []
    for block in body.blocks:
        phis = tuple(
            replace(one, incoming={**one.incoming, **swaps[id(one)]}) if id(one) in swaps else one for one in block.phis
        )
        ops = tuple(
            one
            for op in block.ops
            for one in (
                (*ahead.get(op.at, ()), replace(op, uses=tuple(reads[op.at].get(u, u) for u in op.uses)))
                if op.at in ahead
                else (op,)
            )
        )
        if block.at in added:
            # Before the terminator: nothing runs after a branch, and the
            # copy has to happen on the way out.
            what = _semantics_of_last(ops)
            leaves = what is not None and what.op in (ir.Operation.JUMP, ir.Operation.BRANCH)
            put = tuple(added[block.at])
            ops = (ops[:-1] + put + ops[-1:]) if leaves and ops else (ops + put)
        out.append(replace(block, phis=phis, ops=ops))
    return replace(body, blocks=tuple(out), origin=origin)


def _semantics_of_last(ops: tuple):
    if not ops:
        return None
    one = ops[-1]
    return lower.current(one, node=getattr(one, "node", None))


def colour(body: mir.MirBody, pinned: dict[Value, Register_] | None = None) -> dict[Value, Register_] | str:
    """A register for every value, or why there is not one.

    Greedy over the interference graph, in order of how constrained each
    value is. In SSA that ordering is optimal without a spill loop, because
    the graph is chordal -- but only while nothing is pre-coloured, and
    plenty here is: a barrier pins every register it touches, a call
    clobbers what runtime.py says it does, and `cwd`, `idiv` and a shift by
    cl each demand particular ones. Pre-colouring breaks the guarantee, so
    this can fail, and failing is a refusal to allocate the body rather
    than a licence to guess.

    The identity assignment -- every value back into the register BC used --
    is always valid, since that is where the program came from. So a
    failure here is never "this body cannot be allocated"; it is "this
    body cannot be allocated *the way something asked for*".
    """
    found_live = liveness.live(body)
    graph = interference(body, found_live)
    origin = body.origin

    # The unit is the congruence class, not the value: a phi's members share
    # a register or nothing brings them together. Values no phi touches are
    # their own class, so this is the old behaviour where there are no phis.
    klass = congruent(body)
    of = {one: klass.get(one, one) for one in graph}
    members: dict[Value, set[Value]] = {}
    for one, root in of.items():
        members.setdefault(root, set()).add(one)

    # Class against class. A class two of whose members are live at once is
    # one no single register can hold -- the lost-copy shape, where a value
    # arriving at a phi is still wanted after it. BC's own code contains
    # them and runs, because nothing there has to move; what cannot be done
    # is *relocating* one, since that is where the copy would be needed.
    # So they are recorded and checked against the finished assignment
    # rather than refused on sight, which would refuse the identity too.
    between: dict[Value, set[Value]] = {root: set() for root in members}
    tangled: set[Value] = set()
    for one, others in graph.items():
        for other in others:
            here, there = of[one], of[other]
            if here is there:
                tangled.add(here)
                continue
            between[here].add(there)

    assigned: dict[Value, Register_] = {}
    pinned_roots: set[Value] = set()
    # What the machine requires first, then what the caller asked for. A
    # caller cannot ask for a value to sit anywhere but where its own
    # instruction reads it, so a disagreement is a refusal rather than a
    # preference: see required().
    demanded = required(body)
    for value, want in (pinned or {}).items():
        if demanded.setdefault(value, want) is not want:
            return (
                f"{value} is wanted in {NAMES.get(want, want)} but its operation "
                f"reads it in {NAMES.get(demanded[value], demanded[value])}"
            )
    for value, want in demanded.items():
        root = of.get(value, klass.get(value, value))
        if assigned.setdefault(root, want) is not want:
            return f"{value} is tied to a value that wants {NAMES.get(assigned[root], assigned[root])}"
        pinned_roots.add(root)

    # A class may only sit where every one of its members can be reached
    # from, and 16-bit addressing reaches memory through bx, bp, si or di.
    reached_by = {of.get(one, one) for one in _addressing(body)}
    wide_addressing = {ir.ROOT.get(one, one) for one in target.BASES}
    for root in reached_by & set(assigned):
        want = assigned[root]
        if ir.ROOT.get(want, want) not in wide_addressing:
            return f"{root} is how a cell is reached and {NAMES.get(want, want)} cannot reach one"

    # What each class may not have, because one of its members lives across
    # something that destroys it. Empty on BC's own code -- the call the
    # divide was absorbed from clobbered the same registers, so nothing was
    # ever left in one -- and the reason a hoisted divide cannot simply be
    # coloured: five of lngmix's values cross the pair with only esi and
    # edi surviving it.
    barred: dict[Value, set] = {}
    for one, where in clobbered(body, found_live).items():
        barred.setdefault(of.get(one, one), set()).update(where)

    # Where BC had each class. Congruent by construction, so every member
    # agrees; disagreement means the body did not come from BC and there is
    # no identity to fall back on.
    was: dict[Value, Register_ | None] = {}
    for root, group in members.items():
        seen = {origin[one] for one in group if one in origin}
        was[root] = next(iter(seen)) if len(seen) == 1 else None

    # Identity first, and it is not a heuristic: measured over 27,680 values,
    # none ever interferes with another version of its own register, so
    # putting every value back where BC had it is always a valid colouring.
    # An allocator that moves anything it was not asked to move is emitting
    # copies for nothing, and greedy-by-degree moved 1,274 values doing
    # exactly that. Only a pin, or a clash with one, makes anything move.
    if not any(assigned[root] is not was.get(root) for root in assigned):
        clean = {root: where for root, where in was.items() if where is not None}
        clean.update(assigned)
        if all(
            all(clean[other] is not clean[root] for other in between[root] if other in clean)
            for root in clean
            if root in between
        ):
            return _legal({one: clean[of[one]] for one in graph if of[one] in clean}, barred, of)

    def offers(root: Value) -> list[Register_]:
        out = (
            target.AVAILABLE
            if root not in reached_by
            else [where for where in target.AVAILABLE if ir.ROOT.get(where, where) in wide_addressing]
        )
        gone = barred.get(root)
        return [where for where in out if gone is None or where not in gone]

    # Everything back where BC had it, and then only what conflicts moves.
    # Assigning in degree order and counting only neighbours already
    # assigned let a class processed late find its own register taken and
    # take someone else's, and that one did the same: one forced move
    # displaced twelve values in lngmix, and somewhere down the cascade a
    # value and its readers stopped agreeing.
    wanted: dict[Value, Register_ | None] = dict(assigned)
    for root in between:
        if root in wanted:
            continue
        here = was.get(root)
        wanted[root] = here if here is not None and here in offers(root) else None

    for _ in range(len(between) + 1):
        clash = next(
            (
                (root, other)
                for root, others in between.items()
                for other in others
                if wanted.get(root) is not None and wanted.get(root) is wanted.get(other)
            ),
            None,
        )
        homeless = next((root for root, where in wanted.items() if where is None), None)
        if clash is None and homeless is None:
            break

        # Never the pinned one: a pin is the whole reason an allocation is
        # being made, and moving it answers a different question.
        root = homeless
        if clash is not None:
            root = clash[1] if clash[0] in pinned_roots else clash[0]
            if root in pinned_roots and clash[1] not in pinned_roots:
                root = clash[1]
        if root is None:
            break
        if root in pinned_roots:
            where = NAMES.get(wanted[root], wanted[root])
            return f"{root} and a value it interferes with are both pinned to {where}"

        taken = {wanted.get(other) for other in between[root]}
        free = [where for where in offers(root) if where not in taken]
        if not free:
            return f"{root} interferes with every register at once"
        wanted[root] = free[0]
    else:
        return "the allocation did not settle"

    assigned = {root: where for root, where in wanted.items() if where is not None}

    # A tangled class may stay where it is and may not go anywhere else.
    for root in tangled:
        if assigned.get(root) is not was.get(root):
            here, there = NAMES.get(was.get(root)), NAMES.get(assigned.get(root))
            return f"{root} is wanted across its own phi and cannot move from {here} to {there}"

    # The pins are not checked on the way in, so they are checked here: two
    # of them wanting one register for classes that are live together is a
    # refusal, and without this the loop hands them straight back because a
    # class already assigned is skipped rather than validated.
    for root, others in between.items():
        for other in others:
            if root in assigned and other in assigned and assigned[root] is assigned[other]:
                where = NAMES.get(assigned[root], assigned[root])
                return f"{root} and {other} are live together and both want {where}"
    return _legal({one: assigned[of[one]] for one in graph if of[one] in assigned}, barred, of)


def _legal(where: dict, barred: dict, of: dict):
    """The assignment, or why it is not one.

    The candidate is validated whole rather than trusted from how it was
    built: a pin is written straight into the assignment before `offers`
    is ever asked, so a caller could ask for a value to sit in a register
    the divide it lives across destroys and the greedy would hand it back.
    """
    for value, seat in where.items():
        gone = barred.get(of.get(value, value))
        if gone is not None and seat in gone:
            return f"{value} lives across something that destroys {NAMES.get(seat, seat)}"
    return where


def moved(body: mir.MirBody, assignment: dict[Value, Register_]) -> int:
    """How many values ended up somewhere other than BC put them."""
    return sum(1 for one, where in assignment.items() if where is not body.origin.get(one))
