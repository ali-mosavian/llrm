"""
MIR transforms: a body in, an optimised body out.

Everything else in this project changes a program by patching BC's bytes.
This changes the values, and layout.py turns what is left into bytes -- so a
transform here says what the program computes and nothing about how it was
written. That is the difference the roadmap calls retiring the machine arm.

Long pairs and runtime arithmetic are recognized while raising, before any
transform sees the body.  A transform therefore never reconstructs a source
operation from BC's register convention.

**Every source occurrence stays accounted for.** A transform that deletes
computation leaves an inert operation owning the same opaque identities, or
transfers those identities to its semantic replacement. Byte ranges exist
only in the external source map and are resolved after this tier, in lowering.

Each transform is separately switchable, deliberately. They interact -- a
widened pair changes which loads are redundant -- and a wrong answer from
one is otherwise a bisect through all three.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model.mir import Op
from qbopt.optimize import fill
from qbopt.optimize import peel
from qbopt.analysis import avail
from qbopt.optimize import lcssa
from qbopt.analysis import consts
from qbopt.optimize import profit
from qbopt.optimize import unroll
from qbopt.optimize import promote
from qbopt.model.mir import MirBody
from qbopt.objectfile import module
from qbopt.optimize import strength
from qbopt.model.passes import Where
from qbopt.optimize import algebraic
from qbopt.optimize import loopmotion
from qbopt.optimize import loopsimplify
from qbopt.optimize import pointeraccess
from qbopt.analysis import loops as loopy
from qbopt.model.passes import AddressForm
from qbopt.model.passes import MIRTransform
from qbopt.model.passes import OperationCosts
from qbopt.analysis import liveness as alive_at
from qbopt.analysis.ssa import provider as _provider
from qbopt.analysis.ssa import values as _ssa_values
from qbopt.analysis.ssa import pruned_phis as _pruned_phis
from qbopt.analysis.ssa import substituted as _substituted
from qbopt.model.passes import DEFAULT_MAX_UNROLL_ITERATIONS
from qbopt.model.passes import DEFAULT_MAX_UNROLLED_OPERATIONS


def _absorb(ops: list[Op], gone: set[int]) -> list[Op]:
    """Erase the operations whose address is in ``gone``."""
    return _without(ops, lambda one: one.at in gone)


def _without(ops: list[Op], drop) -> list[Op]:
    """Remove selected computation while retaining exact source ownership.

    A deleted source occurrence becomes an inert marker instead of donating
    a byte interval to a neighbour.  The marker owns the same opaque ids and
    lowers to no instruction; source-free operations disappear completely.
    This preserves disjoint ownership without any pass learning byte ranges.
    """
    out = []
    for op in ops:
        if not drop(op):
            out.append(op)
        elif op.absorbed or op.floating_origin is not None:
            out.append(_empty_operation(op))
    return out


def _phi_reading(phi, swap: dict):
    """One phi taking the provider along any edge that named the load."""
    if not any(one.id in swap for one in phi.incoming.values()):
        return phi
    return replace(phi, incoming={at: _provider(one, swap) for at, one in phi.incoming.items()})


# CSE only ever removes an operation whose whole answer is in its operands.
# Anything that reads memory, writes memory, or is the machine doing
# something -- a call, a push, a branch -- computes from more than its
# arguments, and two of them are not the same computation however alike
# they look.
_PURE = frozenset(
    {
        mir.Kind.ADD,
        mir.Kind.SUB,
        mir.Kind.MUL,
        mir.Kind.FIXED_MUL,
        mir.Kind.SMULHI,
        mir.Kind.PTR_OFFSET,
        mir.Kind.DIV,
        mir.Kind.REM,
        mir.Kind.AND,
        mir.Kind.OR,
        mir.Kind.XOR,
        mir.Kind.SHL,
        mir.Kind.SHR,
        mir.Kind.SAR,
        mir.Kind.NEG,
        mir.Kind.NOT,
        mir.Kind.CONVERT,
        mir.Kind.SIGN_EXTEND,
        mir.Kind.EXTRACT,
        mir.Kind.CONCAT,
        mir.Kind.COPY,
        mir.Kind.ADDRESS,
        mir.Kind.LT,
        mir.Kind.LE,
        mir.Kind.GT,
        mir.Kind.GE,
        mir.Kind.EQ,
        mir.Kind.NE,
        mir.Kind.BELOW,
        mir.Kind.ABOVE,
    }
)


def subexpressions(
    body: MirBody,
    dgroup: frozenset[int] = frozenset(),
    *,
    avoid_store_crossing: bool = False,
) -> MirBody:
    """One operation where two computed the same thing from the same values.

    lngmix is the program this exists for. `s = s + v \\ 7 + v MOD 7`
    divides twice by the same constant, and x86's `idiv` already yields the
    quotient and the remainder from one instruction -- so the second divide
    is not an optimisation BC missed, it is work it did twice.

    The two are not textually equal even so. BC reloads `v`, reconverts it
    and rebuilds the 7, so the second divide names none of the values the
    first one named. Value numbering rather than a syntactic match: a name
    is replaced by what it stands for as the walk reaches it, so a copy
    counts as its source and an operation this has already folded counts as
    the one it folded into. lngmix needs both, three links deep.

    That makes copy propagation part of the question instead of a pass
    before it. What a name stands for is not a rewrite, and materialising
    it as one would leave dead copies for `dead` to find again.

    Refused wherever the second operation's flags are read. In MIR they are
    an ordinary value and substituting them is sound; in the machine they
    are one register that everything in between has already written, so the
    flags of an operation that no longer runs are not there to be read.
    """
    doms = loopy.dominators(list(body.blocks), body.entry)
    order = {block.at: index for index, block in enumerate(body.blocks)}
    whole = _widths(body)
    demanded = halves(body)
    from qbopt.analysis import floatfacts
    from qbopt.analysis import floatbounds

    exact = floatfacts.known(body, dgroup, {}) if any(op.floating for block in body.blocks for op in block.ops) else {}
    bounded = floatbounds.exact(body, exact, dgroup)

    seen: dict[tuple, list[tuple[int, int, Op]]] = {}
    stands: dict[int, mir.Value] = {}  # what a name numbers as -- copies included
    swap: dict[int, mir.Value] = {}  # what a name is rewritten to -- only what folded
    gone: set[int] = set()  # by identity: four of lngmix's ops share address 0x4b
    floating_gone: set[int] = set()
    for block in body.blocks:
        for index, op in enumerate(block.ops):
            stored = _exact_stored_load(op, exact)
            if stored is not None:
                key = _computation(stored, stands, whole)
                if key is not None:
                    seen.setdefault(key, []).append((order[block.at], index, stored))
            source = _copied(op, whole)
            if source is None and op.merges and all((value, HIGH) not in demanded for value in op.merges.values()):
                source = _copied(replace(op, merges={}), whole)
            if source is not None:
                result = op.defines[0]
                source = _provider(source, stands)
                stands[result.id] = source
                if not op.merges or _width(result, op) == 4 or (result, HIGH) not in demanded:
                    swap[result.id] = source
                    gone.add(id(op))
                continue
            # A narrow result carries the prior value's high half as a
            # merge.  That is a real dependency only while somebody reads
            # the preserved half.  When half-liveness proves it dead, the
            # operation's semantic identity is its width-sized arithmetic,
            # not the accidental register-wide read/modify/write form it
            # arrived in.  This is the same general distinction already
            # used for copies above; applying it to every pure computation
            # lets value numbering see repeated descriptor address adds
            # without teaching CSE about a particular register or frontend.
            semantic = (
                replace(op, merges={})
                if op.merges and all((value, HIGH) not in demanded for value in op.merges.values())
                else op
            )
            key = _computation(semantic, stands, whole)
            if key is None:
                continue
            candidates = seen.setdefault(key, [])
            first = next(
                (
                    candidate
                    for candidate in reversed(candidates)
                    if _reaches(candidate[0], candidate[1], order[block.at], index, doms, body, block)
                ),
                None,
            )
            if first is None:
                candidates.append((order[block.at], index, op))
                continue
            at, where, earlier = first
            if op.floating is not None and not _reusable_float_path(
                body, body.blocks[at].at, where, block.at, index, exact, bounded
            ):
                candidates.append((order[block.at], index, op))
                continue
            if op.loads and (
                at != order[block.at] or not _undisturbed(op, earlier, block.ops[where + 1 : index], dgroup)
            ):
                candidates.append((order[block.at], index, op))
                continue
            if op.loads and avoid_store_crossing and any(crossed.stores for crossed in block.ops[where + 1 : index]):
                candidates.append((order[block.at], index, op))
                continue
            if len(earlier.defines) != len(op.defines):
                continue
            if any(one.flags and _read(body, one) for one in op.defines):
                continue
            folded = {mine.id: theirs for mine, theirs in zip(op.defines, earlier.defines)}
            stands.update(folded)
            swap.update(folded)
            (floating_gone if op.floating is not None else gone).add(id(op))

    if not gone and not floating_gone:
        return body
    if floating_gone:
        body = replace(
            body,
            blocks=tuple(
                replace(block, ops=tuple(_erased_floating(op) if id(op) in floating_gone else op for op in block.ops))
                for block in body.blocks
            ),
        )
    if gone:
        body = _reclaimed(body, gone)
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                phis=tuple(_phi_reading(phi, swap) for phi in block.phis),
                ops=tuple(_substituted(op, swap) for op in block.ops),
            )
            for block in body.blocks
        ),
    )


def _reusable_float_path(
    body: MirBody,
    source: int,
    first: int,
    destination: int,
    last: int,
    facts: dict,
    bounded: set[int],
) -> bool:
    from qbopt.model.floating import Exceptions

    blocks = {block.at: block for block in body.blocks}
    candidates = (blocks[source].ops[first], blocks[destination].ops[last])
    deferred = all(op.floating is not None and op.floating.exceptions is Exceptions.DEFERRED for op in candidates)
    predecessors = loopy.predecessors(body.blocks)
    active: set[int] = set()
    checked: dict[int, bool] = {}

    def visit(at: int) -> bool:
        if at in active:
            return False
        if at in checked:
            return checked[at]
        active.add(at)
        ops = blocks[at].ops
        start = first if at == source else 0
        end = last + 1 if at == destination else len(ops)
        exact = all(
            _unchanged_float_environment(op) if deferred else id(op) in bounded or _exact_floating(op, facts)
            for op in ops[start:end]
        )
        parents = predecessors[at]
        result = exact and (at == source or (bool(parents) and all(visit(parent) for parent in parents)))
        active.remove(at)
        checked[at] = result
        return result

    return visit(destination)


def _unchanged_float_environment(op: Op) -> bool:
    from qbopt.model.floating import Exceptions

    if op.barrier or op.kind in (mir.Kind.CALL, mir.Kind.OPAQUE, mir.Kind.FCHECK):
        return False
    if op.floating is None:
        return op.stack is None
    return op.floating.exceptions is Exceptions.DEFERRED


def _exact_stored_load(op: Op, facts: dict) -> Op | None:
    """An exact storage conversion leaves the source value available for reloads."""
    from qbopt.model.floating import Format
    from qbopt.model.floating import Rounding
    from qbopt.model.floating import Precision
    from qbopt.model.floating import Semantics

    if (
        op.kind is not mir.Kind.FSTORE
        or op.floating is None
        or op.floating.inputs != (Format.EXTENDED80,)
        or op.floating.result not in (Format.BINARY32, Format.BINARY64)
        or len(op.stores) != 1
        or op.loads
        or not _exact_floating(op, facts)
    ):
        return None
    (source,) = op.args
    if not isinstance(source, mir.Held) or source.width != 10:
        return None
    rule = Semantics((op.floating.result,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE, op.floating.exceptions)
    return replace(
        op,
        kind=mir.Kind.FLOAD,
        args=(mir.Cell(op.stores[0]),),
        results=(source,),
        defines=(source.value,),
        uses=(),
        loads=op.stores,
        stores=(),
        merges={},
        floating=rule,
    )


def _exact_floating(op: Op, facts: dict) -> bool:
    """No intervening exceptional FP work or unmodelled environment change."""
    from qbopt.analysis import floatfacts

    if op.barrier or op.kind in (mir.Kind.CALL, mir.Kind.OPAQUE):
        return False
    if op.floating is None:
        return op.stack is None
    if op.kind is mir.Kind.FSTORE:
        return (
            len(op.args) == 1
            and isinstance(op.args[0], mir.Held)
            and op.args[0].value in facts
            and floatfacts.evaluated(op.kind, op.floating, (facts[op.args[0].value],)) is not None
        )
    return len(op.results) == 1 and isinstance(op.results[0], mir.Held) and op.results[0].value in facts


def _erased_floating(op: Op) -> Op:
    return replace(
        op,
        op=ir.Operation.NOTHING,
        kind=mir.Kind.NOTHING,
        name="",
        args=(),
        results=(),
        uses=(),
        defines=(),
        loads=(),
        stores=(),
        merges={},
        source_backed=False,
        raised=None,
        floating=None,
        stack=None,
    )


def _reclaimed(body: MirBody, gone: set[int]) -> MirBody:
    """Erase redundant computations without moving source provenance."""
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(_empty_operation(op) if id(op) in gone else op for op in block.ops),
            )
            for block in body.blocks
        ),
    )


def _widths(body: MirBody) -> dict[int, int]:
    """The width each value was defined at.

    A half of one is named `Held(value, 2)` and so is the other half, so an
    operand narrower than its value says which value but not which half.
    nots proved it: `NOTOR` came back with the right low word and the wrong
    high one, because two operations reading opposite halves of the same
    long compared equal. Anything narrow is refused rather than told apart.
    """
    out: dict[int, int] = {}
    for block in body.blocks:
        for op in block.ops:
            for one in op.results:
                if isinstance(one, mir.Held):
                    out.setdefault(one.value.id, one.width)
    changing = True
    while changing:
        changing = False
        for block in body.blocks:
            for phi in block.phis:
                if phi.result.id in out or not phi.incoming:
                    continue
                widths = {out.get(value.id) for value in phi.incoming.values()}
                if len(widths) == 1 and None not in widths:
                    out[phi.result.id] = next(iter(widths))
                    changing = True
    return out


def _full(one, whole: dict[int, int]) -> bool:
    """Whether this operand names the whole of its value, not one half."""
    return whole.get(one.value.id) == one.width


def _copied(op: Op, whole: dict[int, int]) -> mir.Value | None:
    """The value this operation is another name for, if it is only that."""
    if op.kind not in (mir.Kind.COPY, mir.Kind.LOAD) or len(op.defines) != 1:
        return None
    if op.loads or op.stores or op.merges:
        return None
    held = [one for one in op.args if isinstance(one, mir.Held)]
    if len(held) != 1 or len(op.args) != 1:
        return None
    if held[0].width != _width(op.defines[0], op) or not _full(held[0], whole):
        return None
    return held[0].value


def _width(_value: mir.Value, op: Op) -> int | None:
    """The width this operation's result comes out at, or None."""
    for one in op.results:
        if isinstance(one, mir.Held):
            return one.width
    return None


def _computation(op: Op, stands: dict[int, mir.Value], whole: dict[int, int]) -> tuple | None:
    """What this operation computes, or None where that is not only its operands."""
    floating = op.floating is not None and op.kind in (
        mir.Kind.FLOAD,
        mir.Kind.FADD,
        mir.Kind.FSUB,
        mir.Kind.FMUL,
        mir.Kind.FDIV,
        mir.Kind.FSQRT,
    )
    if (op.kind not in _PURE | {mir.Kind.LOAD} and not floating) or op.stores or op.merges or op.barrier:
        return None
    if not op.defines or not op.args:
        return None
    if set(op.loads) != {arg.ref for arg in op.args if isinstance(arg, mir.Cell)}:
        return None
    named = []
    for one in op.args:
        if isinstance(one, mir.Held):
            if not _full(one, whole):
                return None
            named.append(("v", stands.get(one.value.id, one.value).id, one.width))
        elif isinstance(one, mir.Const):
            named.append(("c", one.n, one.width))
        elif isinstance(one, mir.Symbol):
            named.append(("s", one))
        elif isinstance(one, mir.FrameAddress):
            named.append(("a", one))
        elif isinstance(one, mir.Cell):
            ref = mir._symbolic_ref(one.ref)
            if not mir.same_bytes(ref, ref):
                return None
            named.append(("m", ref))
        else:
            return None
    results = tuple(one.width for one in op.results if isinstance(one, mir.Held))
    operands = (
        frozenset(named)
        if len(named) == 2
        and op.kind
        in {
            mir.Kind.ADD,
            mir.Kind.MUL,
            mir.Kind.AND,
            mir.Kind.OR,
            mir.Kind.XOR,
            mir.Kind.EQ,
            mir.Kind.NE,
        }
        else tuple(named)
    )
    return (op.kind, op.floating if floating else op.name, operands, results)


def _reaches(at: int, where: int, then: int, index: int, doms, body, block) -> bool:
    """Whether the earlier operation has certainly run by the later one."""
    if at == then:
        return where < index
    return body.blocks[at].at in doms.get(block.at, frozenset())


def _read(body: MirBody, value: mir.Value) -> bool:
    """Whether anything in the body uses this value."""
    return any(
        value in op.uses or any(isinstance(one, mir.Held) and one.value is value for one in op.args)
        for block in body.blocks
        for op in block.ops
    )


def reused_divides(body: MirBody, dgroup: frozenset[int], found=None) -> MirBody:
    """A divide whose answers the divide before it already computed.

    One idiv yields the quotient and the remainder together, and BC asks
    for them with two calls -- so lngmix's `s = s + v \\ 7 + v MOD 7`
    divides the same two numbers ten times a loop for an answer it is
    holding. The second divide becomes a copy of the first's answer.

    Only its answers are substituted, and only where everything else it
    defined is dead. A folded call defines a value per register it
    clobbers, and those are not results: reading them as "what the divide
    before it left there" would be reasoning about registers, which is not
    this layer's to do. Where one is live the pair is refused instead.

    The copy stays where the divide was and keeps its identity, so the
    bytes it stood for are still accounted for by it. What follows it --
    the operation that hands the answer's high half back -- reads the same
    value it always did and needs no changing at all.

    Nothing outside the body is touched. Dropping the site's own record
    from the module -- which is what says to emit a divide there -- looks
    like the tidy thing and is not: a body the allocator refuses is laid
    out as it was *raised*, and that body still holds the divide. With the
    record gone it emitted as the bare call BC wrote, its push run already
    folded away, and lngmix stopped early under DOSBox. What an operation
    is, is the operation's to say: asm compares its kind against the one
    its site raises as, and the copy no longer matches.
    """
    pairs_found = divided_twice(body, dgroup)
    if not pairs_found:
        return body
    alive = live(body)
    into: dict[int, Op] = {}
    for _at, earlier, one in pairs_found:
        if len(one.results) != 2 or len(earlier.results) != 2:
            continue
        served, wanted = None, None
        for mine, theirs in zip(one.results, earlier.results):
            if mine.value in alive:
                served, wanted = theirs, mine
        if served is None or wanted is None:
            continue
        # One register is what a copy writes, so one value is all it may
        # define for real. Every other value the site defined -- the
        # answer it was not asked for as much as the registers it merely
        # clobbered -- has to be dead, or the copy hands a reader a
        # register holding something else: the other answer's own name
        # says quotient and ebx would still hold the first divide's
        # remainder. They stay on `defines` because a definition is what
        # ends a live range and a phi naming one still has to find one --
        # dead, that is bookkeeping; live, it would be a lie.
        if any(value in alive for value in one.defines if value is not wanted.value):
            continue
        into[id(one)] = replace(
            one,
            kind=mir.Kind.COPY,
            op=ir.Operation.MOVE,
            name="mov",
            # Every value the site defined stays defined here. The copy
            # writes one of them and the rest are dead, but a definition
            # is what ends a live range: dropping them let the register
            # the first divide's answer sits in stay live across this,
            # and the allocator refused the body with two values pinned
            # to the same register. A phi naming one of them also has to
            # go on finding a definition.
            defines=one.defines,
            uses=(served.value,),
            loads=(),
            stores=(),
            merges={},
            args=(served,),
            results=(wanted,),
            source_backed=False,
        )
    if not into:
        return body
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(into.get(id(op), op) for op in block.ops),
            )
            for block in body.blocks
        ),
    )


def divided_twice(body: MirBody, dgroup: frozenset[int]) -> "list[tuple[int, Op, Op]]":
    """Each divide whose answers the divide before it already computed.

    lngmix is `s = s + v \\ 7 + v MOD 7`: BC calls B$DVI4 and then B$RMI4
    over the same two operands, and one idiv computes the quotient and the
    remainder together. So the second site is not work -- it is the answer
    the first one has -- and this is the pair, as (which block, first,
    second).

    Four things have to hold, and each of them is a way the second could
    be a different computation from the first:

    - the same operands. A cell is compared by the bytes it names, which
      is what makes two separately raised operands equal at all.
    - nothing may have written those cells in between. A store that could
      land on one, and any call or barrier -- whose memory is its own --
      ends it.
    - the same block. What reaches the second one along another edge is a
      question this does not ask, so it does not look across one.

    Nothing here asks what was written to a register in between, and it
    would be wrong to: the first divide's answers are values, and a value
    is written once. Keeping them until the second site reads them is the
    allocator's, not this pass's -- asking it here was a physical-register
    test wearing an SSA variable's name.
    """
    found: list[tuple[int, Op, Op]] = []
    for block in body.blocks:
        for index, one in enumerate(block.ops):
            if one.kind is not mir.Kind.DIVMOD:
                continue
            earlier = next(
                (
                    other
                    for other in reversed(block.ops[:index])
                    if other.kind is mir.Kind.DIVMOD and other.args == one.args
                ),
                None,
            )
            if earlier is None:
                continue
            between = block.ops[block.ops.index(earlier) + 1 : index]
            if not _undisturbed(one, earlier, between, dgroup):
                continue
            found.append((block.at, earlier, one))
    return found


def _undisturbed(one: Op, earlier: Op, between: list, dgroup: frozenset[int]) -> bool:
    """Whether the second divide still reads what the first one read."""
    cells = [arg.ref for arg in one.args if isinstance(arg, mir.Cell)]
    for other in between:
        if other.barrier or other.kind in (mir.Kind.CALL, mir.Kind.ESCAPE):
            return False
        if any(mir.overlapping(ref, wrote, dgroup) for ref in cells for wrote in other.stores):
            return False
    return True


def placed(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """Anything standing inside a call's argument run, moved ahead of it.

    BC writes a call's arguments as a run of pushes and then the call, and
    absorbing one deletes that whole region -- push through call, which is
    what makes absorption smaller than what BC wrote. So an instruction
    standing in the middle of the run is what stops the call being absorbed
    at all: taking it out with the region would delete real work.

    lngmix is the shape. `s = s + v \\ 7 + v MOD 7` computes the quotient,
    stores its two halves, and then pushes the same operands again for the
    remainder -- and those two stores sit between the second run's first
    push and its call. The site is refused, the loop keeps a runtime call,
    and the hoist will not touch a loop that holds one.

    The stores do not depend on the run: they hold the *previous* divide's
    results, so they can go before it. Then the run is contiguous, the site
    absorbs on the next round -- rewrite.py iterates to a fixed point -- and
    both divides become one operation over the same operands, which is one
    key twice and what cse is waiting for.

    Moved ahead rather than behind. An argument's own value is read where
    the push stands, so nothing may pass it in the other direction.
    """
    out = []
    changed = False
    for block in body.blocks:
        ops = list(block.ops)
        for index in range(len(ops) - 1, -1, -1):
            if ops[index].kind is not mir.Kind.CALL:
                continue
            run = _argument_run(ops, index, dgroup, calls)
            if run is None:
                continue
            first, standing = run
            kept = [one for at, one in enumerate(ops) if at not in standing]
            ahead = [ops[at] for at in sorted(standing)]
            ops = kept[:first] + ahead + kept[first:]
            changed = True
        out.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(out)) if changed else body


def _argument_run(ops: list[Op], call: int, dgroup: frozenset[int], calls: dict[int, str]):
    """(where the run starts, which of its operations do not belong to it).

    Walked back from the call, taking pushes and anything that may pass
    them, and stopping at the first thing that may not. None where no push
    was reached, or where nothing stands among them.

    What stands is usually *after* the last push rather than between two of
    them -- BC computes, pushes for the next call, then stores what it
    computed -- so this cannot wait for a push before it starts collecting.
    """
    first, standing, pushes = call, set(), 0
    at = call - 1
    while at >= 0:
        one = ops[at]
        if one.kind is mir.Kind.ARG:
            pushes += 1
            first, at = at, at - 1
            continue
        if not _may_pass(one, [ops[x] for x in range(at + 1, call)], dgroup, calls):
            break
        standing.add(at)
        first, at = at, at - 1
    if not pushes or not standing:
        return None
    # Only what stands among the pushes. Anything collected before the
    # first one is not in the run and has no reason to move.
    lowest = min(x for x in range(first, call) if ops[x].kind is mir.Kind.ARG)
    standing = {x for x in standing if x > lowest}
    return (lowest, standing) if standing else None


def _may_pass(one: Op, run: list[Op], dgroup: frozenset[int], calls: dict[int, str]) -> bool:
    """Whether this operation can move ahead of the run standing after it."""
    if one.barrier or one.kind in _OBSERVED or one.at in calls:
        return False
    made = {value for other in run for value in other.defines}
    if any(use in made for use in one.uses):
        return False
    wrote = {value for value in one.defines}
    if any(use in wrote for other in run for use in other.uses):
        return False
    # A push moves the stack pointer, so `mov bp,sp` may not pass one.
    written, read = mir.unheld(one)
    for other in run:
        theirs_written, theirs_read = mir.unheld(other)
        if _meets(written, theirs_read) or _meets(read, theirs_written):
            return False
    for ref in one.loads + one.stores:
        for other in run:
            for theirs in other.loads + other.stores:
                if mir.overlapping(ref, theirs, dgroup):
                    return False
    return True


def _meets(one: frozenset | None, other: frozenset | None) -> bool:
    """Whether two sets share anything, None being everything."""
    if one is None or other is None:
        return one != frozenset() and other != frozenset()
    return bool(one & other)


def _empty_operation(op: Op) -> Op:
    """Keep opaque source ownership, but no computation or memory effect."""
    return replace(
        op,
        op=ir.Operation.NOTHING,
        name="",
        kind=mir.Kind.NOTHING,
        defines=(),
        uses=(),
        array=None,
        memory_values=(),
        floating=None,
        floating_origin=None,
        args=(),
        results=(),
        loads=(),
        stores=(),
        merges={},
        source_backed=False,
        raised=None,
        target=None,
        cases=(),
        symbol=False,
        args_known=True,
        memory_complete=True,
        reads_complete=True,
        opaque_defs=frozenset(),
        opaque_uses=frozenset(),
        stack=None,
        test=None,
        indirect=False,
    )


def without_dead_stores(
    body: MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    private=None,
    bounds: dict | None = None,
    handles_errors: bool = True,
) -> MirBody:
    """Every store overwritten, or never observable, before anything read it, removed."""
    gone = {id(op) for op in avail.dead_stores(body, dgroup, calls, private, bounds, handles_errors)}
    if not gone:
        return body
    return replace(
        body,
        blocks=tuple(
            replace(
                one,
                ops=tuple(_without(list(one.ops), lambda op: id(op) in gone)),
            )
            for one in body.blocks
        ),
    )


# A root register at the width an operand reads it. ir.ROOT maps the narrow
# name to the wide one; this is the way back, and only for the general
# registers -- a segment register has no narrower form and is never a
# provider here.


def forwarded(
    body: MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    *,
    avoid_store_crossing: bool = False,
) -> MirBody:
    """Replace known memory operands with SSA values, extending their uses.

    Arithmetic remains intact. The allocator, not this pass, decides where
    the longer-lived provider resides.
    """
    want = frozenset(op.at for block in body.blocks for op in block.ops if op.loads)
    if not want:
        return body
    served = {id(one.op): one.value for one in avail.forwardable(body, dgroup, calls, want) if one.value is not None}
    if avoid_store_crossing:
        locations = {id(op): (block.at, index) for block in body.blocks for index, op in enumerate(block.ops)}
        definitions = {
            value: (block.at, index)
            for block in body.blocks
            for index, op in enumerate(block.ops)
            for value in op.defines
        }
        by_at = {block.at: block for block in body.blocks}
        op_by_id = {id(op): op for block in body.blocks for op in block.ops}
        predecessors = loopy.predecessors(body.blocks)

        def blocks_reaching(destination: int) -> set[int]:
            reached = {destination}
            work = [destination]
            while work:
                reached.update(new := predecessors[work.pop()] - reached)
                work.extend(new)
            return reached

        def crosses_store(op: Op, holder) -> bool:
            if not isinstance(holder, mir.Value):
                return False
            source = definitions.get(holder)
            destination = locations[id(op)]
            if source is None:
                return True
            if source[0] == destination[0]:
                return source[1] >= destination[1] or any(
                    one.stores for one in by_at[source[0]].ops[source[1] + 1 : destination[1]]
                )
            reaching = blocks_reaching(destination[0])
            if source[0] not in reaching:
                return True
            seen: set[int] = set()
            work = [source[0]]
            arrived = False
            while work:
                at = work.pop()
                if at in seen or at not in reaching:
                    continue
                seen.add(at)
                block = by_at[at]
                low = source[1] + 1 if at == source[0] else 0
                high = destination[1] if at == destination[0] else len(block.ops)
                if any(one.stores for one in block.ops[low:high]):
                    return True
                if at == destination[0]:
                    arrived = True
                else:
                    work.extend(block.succ)
            return not arrived

        served = {
            identity: holder for identity, holder in served.items() if not crosses_store(op_by_id[identity], holder)
        }
    if not served:
        return body

    out = []
    for block in body.blocks:
        ops: list[Op] = []
        for op in block.ops:
            holder = served.get(id(op))
            if isinstance(holder, (mir.Const, mir.Symbol)) and op.kind is not mir.Kind.LOAD:
                holder = None
            args = _served(op, holder) if holder is not None else None
            if args is None:
                ops.append(op)
            elif isinstance(holder, (mir.Const, mir.Symbol)):
                # Only a load, which becomes the constant itself. An
                # arithmetic operand is a machine question this is not
                # allowed to answer: `idiv [x]` has no immediate form, and
                # serving its operand refused the whole of deedlines.
                # A load always has one -- it is a materialisation.
                #
                # A constant is not a value either: nothing holds it and
                # nothing has to stay alive to, so `uses` does not grow.
                ops.append(
                    replace(
                        op,
                        kind=mir.Kind.COPY,
                        op=ir.Operation.MOVE,
                        name="mov",
                        args=args,
                        loads=(),
                        uses=op.uses,
                        source_backed=False,
                        raised=None,
                        symbol=False,
                    )
                )
            elif op.kind is mir.Kind.LOAD:
                # A load served by a value is a copy of it. Left a load, the
                # counter's `mov ax,[x]` hid PLASMA's x from induction.
                ops.append(
                    replace(
                        op,
                        kind=mir.Kind.COPY,
                        op=ir.Operation.MOVE,
                        name="mov",
                        args=args,
                        loads=(),
                        uses=op.uses + (holder,),
                        source_backed=False,
                        raised=None,
                        symbol=False,
                    )
                )
            else:
                ops.append(replace(op, args=args, loads=(), uses=op.uses + (holder,)))
        out.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(out))


def _served(op: Op, holder) -> "tuple[mir.Arg, ...] | None":
    """`op`'s one memory source read from whatever holds `holder` instead.

    A value, not a register: which one holds it is the allocator's answer,
    and naming one here is what rule 5 forbids. This asked
    `_at_width(root, width)` and wrote the register down, which was
    forward.py's 22 machine references in one line.
    """
    if op.floating is not None:
        return None
    cells = [one for one in op.args if isinstance(one, mir.Cell)]
    if len(cells) != 1:
        return None
    cell = cells[0]
    if cell in op.results:
        # An update in place: the read is the write's own operand, and serving
        # it turns one instruction into a load, the operation and a store.
        return None
    if isinstance(holder, (mir.Const, mir.Symbol)):
        if holder.width != cell.ref.width:
            return None
        return tuple(holder if one is cell else one for one in op.args)
    return tuple(mir.Held(holder, cell.ref.width) if one is cell else one for one in op.args)


def _preheader(body: MirBody, loop) -> int | None:
    """The block a loop is entered through, where there is exactly one.

    A hoisted operation has to run once before the loop and on every path
    into it, so it goes in the block that dominates the header from outside
    -- and only where there is one of those. Two entries into a loop is a
    header with two outside predecessors, and synthesising a block for it is
    a bigger change than any pass here needs; those loops are refused.
    """
    outside = [block.at for block in body.blocks if loop.header in block.succ and block.at not in loop.body]
    return outside[0] if len(outside) == 1 else None


def _effective(body: MirBody, calls: dict[int, str]) -> set:
    """Every value some instruction reads, rather than merely preserves.

    Transitive through phis, because the chain that matters is: `imul word
    [k]` preserves edx's high half, the latch's phi carries that, and the
    next iteration's imul preserves it again. Nothing reads it, and yet
    every operation in the run appeared to depend on the one before it
    across the back edge.

    Grown from what is definitely read rather than shrunk from everything,
    so a cycle cannot talk itself into being effective.
    """
    wanted: set = set()
    carrying: dict = {}
    for block in body.blocks:
        for phi in block.phis:
            for value in phi.incoming.values():
                carrying.setdefault(phi.result, set()).add(value)
        for op in block.ops:
            # `merges` records a value preserved into a narrow result, not
            # necessarily an unconsumed one.  `and ax,ax`, for example,
            # reads AX to set the flags and also preserves the upper half of
            # EAX; dropping every merged use therefore made an invariant
            # condition load look dead.  `mir.consumed()` is the one
            # machine-neutral definition of the distinction: it retains an
            # explicit operand (and an address base) even when the same
            # value is merged, while excluding preservation alone.
            wanted |= _consumed(op)
    changing = True
    while changing:
        changing = False
        for result, incoming in carrying.items():
            if result in wanted and not incoming <= wanted:
                wanted |= incoming
                changing = True
    return wanted


def _starts(phis: list) -> set:
    """Values a phi in this loop carries in from somewhere.

    A definition whose value reaches one begins something the loop goes on
    to change, and moving it out means the next pass sees where the last
    one finished.
    """
    return {value for phi in phis for value in phi.incoming.values()}


def _rewritten(ops: list[Op], phis: list) -> set:
    """Values a phi carries that the loop goes on to define again.

    A definition may only leave a loop if it is the only one of its variable
    in there. A move that starts an inner counter reads nothing the outer
    loop writes, so it is invariant by every other test here -- and hoisting
    it means the second pass of the outer loop starts from where the inner
    one left off. segld printed 1030 for 1050, one inner loop short.

    In SSA a variable written twice in a loop is a phi with an incoming
    defined inside it, which is the same question asked of values. It used
    to count definitions per register through `origin`, which is the
    machine's account of which of them are one variable.
    """
    inside = {value for one in ops for value in one.defines if not value.flags}
    out: set = set()
    for phi in phis:
        coming = set(phi.incoming.values())
        if coming & inside:
            out |= coming
    return out


def _cannot_fault(op: Op) -> bool:
    """Whether this divide can be performed where it might not have been.

    `idiv` faults on a zero divisor and on the one quotient that does not
    fit -- the most negative dividend over -1. Both are the divisor, and
    both are answerable only where it is written down: a cell's contents
    are whatever the loop's last iteration left there.
    """
    if op.kind is not mir.Kind.DIVMOD or len(op.args) != 2:
        return False
    divisor = op.args[1]
    return isinstance(divisor, mir.Const) and consts.masked(divisor.n, divisor.width) not in (
        0,
        (1 << (divisor.width * 8)) - 1,
    )


def _whole_shift(op: Op, readable: set | None) -> bool:
    """A complete scalar definition needs no loop-carried destination contents."""
    match op.kind, op.args, op.results:
        case mir.Kind.SHL, (mir.Held(width=width), mir.Const(n=count)), (mir.Held(width=result_width),):
            return (
                width == result_width
                and 0 < count < width * 8
                and not op.merges
                and readable is not None
                and not any(value.flags and value in readable for value in op.defines)
            )
        case _:
            return False


def _complete_value(op: Op, readable: set | None) -> bool:
    """Whether this pure operation defines one complete, reparentable value.

    An SSA result that merely initializes a nested recurrence is immutable;
    the recurrence's later versions cannot change it. LICM may therefore
    move a complete computation of outer-invariant operands and give its
    crossing result a fresh variable. A COPY remains excluded: source-level
    counter resets deliberately execute once per enclosing iteration, and
    moving one was the historical SEGld miscompile this distinction protects.
    """
    if (
        readable is None
        or op.kind in (mir.Kind.COPY, mir.Kind.ADD_CARRY, mir.Kind.SUB_BORROW, mir.Kind.DIVMOD, mir.Kind.UDIVMOD)
        or op.loads
        or op.stores
        or op.merges
        or op.barrier
        or op.floating is not None
        or op.stack is not None
        or len(op.results) != 1
        or not isinstance(op.results[0], mir.Held)
    ):
        return False
    result = op.results[0].value
    values = {value for value in op.defines if not value.flags}
    return values == {result} and not any(value.flags and value in readable for value in op.defines)


_consumed = mir.consumed


def _invariant_run(
    ops: list[Op],
    carried: set,
    stores: list,
    dgroup: frozenset[int],
    calls: dict[int, str],
    phis: list,
    bounds: dict | None = None,
    starts: set | None = None,
    readable: set | None = None,
    intervals: dict | None = None,
    nonempty: bool = False,
    floating_allowed: frozenset[int] = frozenset(),
) -> list[Op]:
    """The ops in this loop whose result never changes, in order.

    Grown rather than filtered: an op is invariant when every cell it reads
    is one no store in the loop can reach, and every register it reads was
    defined outside the loop or by an op already in the run. That second
    clause is why this is a fixed point and not a scan.

    A loop holding a call is refused whole -- a call's memory is its own,
    so nothing in the loop can be shown to read a cell it cannot reach.
    Asked of the address rather than of the operation, that refused every
    loop BC had ever put a runtime call in, absorbed or not: lngmix's
    divide folds to a copy and the copy still stands at `B$RMI4`'s
    address. What an operation is, is the operation's to say.
    """
    # An opaque machine barrier makes every unmodelled resource observable,
    # so no operation may cross it.  A source-language volatile access is
    # different: its explicit memory footprint is complete, and only that
    # access must remain ordered with the other volatile accesses.  Pure,
    # nonvolatile work on proven-disjoint storage may move around it without
    # changing the observable volatile sequence.  Treating both as the same
    # blanket refusal kept C's immutable frame arguments inside every
    # volatile loop even though their cells cannot alias the volatile object.
    if any(one.kind in (mir.Kind.CALL, mir.Kind.ESCAPE) or (one.barrier and not one.volatile) for one in ops):
        return []
    made: set = set()
    run: list = []
    twice = _rewritten(ops, phis)
    begins = starts or set()
    changing = True
    while changing:
        changing = False
        for one in ops:
            real = mir.instruction(one)
            if (
                one in run
                or one.volatile
                or one.stores
                or (one.floating is not None and id(one) not in floating_allowed)
                or not real
            ):
                continue
            # A branch is where the loop is. hotlop's latch block held
            # `cmp`, `jle` and `jmp`, all three reading nothing the loop
            # writes, so all three were invariant by the test above and all
            # three left -- and the back edge left with them.
            if one.kind in (mir.Kind.JUMP, mir.Kind.BRANCH):
                continue
            # Nor a divide that could trap. A preheader runs whether the
            # body does or not -- BC writes its loops rotated, entering at
            # the test -- so hoisting one onto a zero-trip path raises a
            # division BC never performed. Only a divisor written down and
            # known to be neither of the two that fault leaves.
            if one.kind in (mir.Kind.DIVMOD, mir.Kind.UDIVMOD) and not _cannot_fault(one):
                continue
            # Nor a move. The guard was "every source is a register", so a
            # constant load was real work and could leave; with the hoist
            # no longer allocating, letting one leave hoists the counter's
            # own initialiser and hotlop printed 0 for 630 on nine of
            # twelve configurations. Every copy stays until regalloc can
            # place what crosses the edge.
            # Nor anything that computes nothing. A register-to-register
            # move is invariant whenever its source is, so hoisting one and
            # putting a split back in its place is churn -- and the split
            # comes back through emission as an ordinary move, is hoisted in
            # turn, and hotlop's loop body became eighteen `mov cx,bx` in
            # two rounds. What this is for is moving work: a load, or an
            # operation over one.
            # Nor anything that computes nothing. A register-to-register
            # move is invariant whenever its source is, so hoisting one and
            # putting a split back in its place is churn -- and the split
            # comes back through emission as an ordinary move, is hoisted in
            # turn, and hotlop's loop body became eighteen `mov cx,bx` in
            # two rounds. What this is for is moving work: a load, or an
            # operation over one.
            #
            # This catches `add bx,ax` too, which is not a move and is real
            # work, and letting it through looks like a plain bug -- press
            # accumulates four invariant products into bx and with the adds
            # refused each product crosses into the loop wanting a register
            # of its own. It is not a bug. A long add is `add` then `adc`,
            # joined by the carry and by nothing this can see, and with the
            # pair split across the loop edge lngmix printed 1185033780 for
            # 142900. Whatever admits the adds has to keep them together.
            if any(isinstance(result, mir.Opaque) for result in one.results):
                continue
            if (
                one.kind is mir.Kind.COPY
                and not one.loads
                and not any(use in made for use in one.uses)
                and not (len(one.args) == 1 and isinstance(one.args[0], mir.Symbol))
                and not _literal(one)
            ):
                continue
            # Only definition of its register in the loop, or the loop's own
            # Both, and neither alone. "A phi carries it" refuses harr's
            # `mov si,0`, which is safe: si is the array base, a phi carries
            # it because it is live around the loop, and nothing writes it
            # again. "The register is written twice" refuses hotlop's load
            # of `n`, also safe: the loop writes ax on every line and the
            # load is consumed where it stands. What is unsafe is the pair --
            # a value a phi carries whose register the loop goes on to
            # change, which is segld's inner counter and 1030 for 1050.
            # A stable load can replace its carried initial value only after
            # proving that at least one iteration executes.
            if (
                any(
                    value in begins and value in twice and (readable is None or value in readable)
                    for value in one.defines
                    if not value.flags
                )
                and not (
                    one.kind is mir.Kind.COPY
                    and not one.merges
                    and len(one.args) == 1
                    and isinstance(one.args[0], mir.Symbol)
                )
                and not _whole_shift(one, readable)
                and not _complete_value(one, readable)
                and not (nonempty and one.loads and not one.merges)
            ):
                continue
            # An operand nothing writes down used to end the run here.
            # hotlop hoisted `mov ax,[n] / imul word [k]`, the recolour
            # renamed the load to cx, and the multiply went on reading ax:
            # 0 for 630. Two things had to exist before that could be
            # allowed. regalloc.required() now refuses an assignment that
            # puts such an operand anywhere but where its instruction reads
            # it, and a result the machine places is copied out rather than
            # re-seated -- see `copied` below.
            # Each side with its own facts: a POKE's selector is a value at
            # the store, and asked with the load's alone the store could be
            # in any segment, so no descriptor load in a DEF SEG loop left.
            if any(
                ref.addr is None
                or mir.overlapping(ref, other, dgroup, bounds, known=(intervals or {}).get(id(one)), other_known=theirs)
                for ref in one.loads
                for other, theirs in stores
            ):
                continue
            # A phi result is the loop-carried value itself: `v2` at
            # hotlop's header is the counter. Collecting only op.defines
            # left every phi looking like something defined outside, so a
            # compare against the counter read as invariant.
            inside = {value for other in ops for value in other.defines} | carried
            # Per use, not per value: hotlop's counter is genuinely read by
            # the compare, and that must not make the load of `n` -- which
            # only preserves the register it lives in -- loop-carried too.
            blocking = [use for use in _consumed(one) if not use.flags]
            if any(use in inside and use not in made for use in blocking):
                continue
            run.append(one)
            made.update(one.defines)
            changing = True

    thinning = True
    while thinning:
        thinning = False
        for one in run:
            if (
                one.kind is not mir.Kind.COPY
                or one.loads
                or any(isinstance(arg, mir.Symbol) for arg in one.args)
                or _literal(one)
            ):
                continue
            if any(value in other.uses for other in run if other is not one for value in one.defines):
                continue
            run = _pruned(run, set(one.defines))
            thinning = True
            break
    return run


def _pruned(run: list, drop: set) -> list:
    """The run without the values named, and without whatever fed only them.

    Dropping the operation that defines a value leaves anything reading it
    computing from something no longer there, so this runs to a fixed point
    rather than filtering once.
    """
    keep = [one for one in run if not (set(one.defines) & drop)]
    changing = True
    while changing:
        changing = False
        gone = {value for one in run if one not in keep for value in one.defines}
        for one in list(keep):
            if any(use in gone for use in one.uses):
                keep.remove(one)
                changing = True
    return keep


def _crossing(run: list, rest: list, phis: list | None = None, wanted: set | None = None) -> frozenset | None:
    """The values the run computes that the rest of the loop still reads.

    Each needs a register of its own, so this used to insist on exactly one.
    The count is not the constraint, the registers are, and the caller
    finds that out by asking the allocator. And no flag among them:
    a flag cannot be carried to the loop in a register, so a comparison
    whose answer is read inside it has to stay inside it. Counting only
    non-flag values here is what let hotlop's compare move out from under
    the branch that reads it.
    """
    crossing = _crossed_values(run, rest, phis, wanted)
    if not crossing or any(value.flags for value in crossing):
        return None
    return frozenset(crossing)


def _crossed_values(run: list, rest: list, phis: list | None, wanted: set | None) -> set:
    # A phi carries a value out of the run as surely as an instruction
    # reads one, and `rest` holds no phis: hotlop's high half reached the
    # next iteration that way, seen by nothing here.
    taken = {value for other in rest for value in other.uses} | {
        value for phi in (phis or ()) for value in phi.incoming.values()
    }
    if wanted is not None:
        taken &= wanted
    return {value for one in run for value in one.defines if value in taken}


# Operations the body can be observed through, whatever they define. A
# store leaves a mark, a call and the control transfers take the program
# somewhere, and everything touching the stack moves sp. The x87 forms are
# here because their effect is on a stack this layer does not model.
#
# JOIN is deliberately not here. The list used to name ir.Operation.RESTORE
# and that entry never once matched: the raise gives a restore its own
# Synth op, so the name it was compared against was never the name it had.
# Adding it for real cost 1,134 bytes over the corpus and 20 deletions --
# a join whose result nothing reads is as dead as anything else.
_OBSERVED = frozenset(
    {
        mir.Kind.CALL,
        mir.Kind.RETURN,
        mir.Kind.JUMP,
        mir.Kind.BRANCH,
        mir.Kind.SWITCH,
        mir.Kind.ESCAPE,
        mir.Kind.ARG,
        mir.Kind.RESULT,
        mir.Kind.OPAQUE,
        mir.Kind.FLOAD,
        mir.Kind.FSTORE,
        mir.Kind.FADD,
        mir.Kind.FSUB,
        mir.Kind.FMUL,
        mir.Kind.FDIV,
        mir.Kind.FNEG,
        mir.Kind.FABS,
        mir.Kind.FSQRT,
        mir.Kind.FCOMPARE,
        mir.Kind.FCHECK,
    }
)


def _leaving(body: MirBody) -> set:
    return set(mir.exposed(body))


# A value is a 32-bit register and this machine's code is 16-bit, so the
# two halves of one are read and written independently. LOW is what a
# narrow operand names; HIGH is what a narrow write leaves alone.
LOW, HIGH = 0, 1


def halves(body: MirBody) -> set:
    """Which half of which value something reads, to a fixed point.

    Whole-value liveness cannot answer the question this machine asks. Every
    narrow write is a read-modify-write here -- `mov bx,2EEh` writes bx and
    preserves the top half of ebx, so it reads the ebx before it -- and
    counting that as reading the value keeps the previous write alive for
    ever. press's `mov bx,cx`, overwritten two bytes later, was live on the
    strength of a half nothing wanted.

    So the unit is (value, half). A narrow write takes HIGH from the value
    it overwrites and nothing else, which is what makes a chain of them
    collapse: each link is read only for a half, and where no one ever reads
    that half the whole chain is dead.

    Both halves of everything reaching an exit are live, because what the
    caller reads is not a fact this body holds.
    """
    out: set = set()
    for value in _leaving(body):
        out.add((value, LOW))
        out.add((value, HIGH))

    def widths(op: Op) -> dict:
        """The widest each value is read at by this operation.

        By value, not by register root through `origin`. Asked the old way,
        an operand a pass had rewritten to name a value was not an ir.Reg
        and so matched no root -- and a folded `mov eax,186A0h` whose only
        reader was such an operand looked unread. lngmix printed 110 for
        142900: the dividend was deleted and idiv divided whatever was in
        eax.
        """
        found: dict = {}
        for one in op.args:
            if isinstance(one, mir.Held):
                found[one.value] = max(found.get(one.value, 0), one.width)
        for ref in op.loads + op.stores:
            if ref.base is not None:
                found[ref.base] = max(found.get(ref.base, 0), ref.base_width)
        return found

    changing = True
    while changing:
        before = len(out)
        for block in body.blocks:
            for op in block.ops:
                if not _kept(op) and not any((one, half) in out for one in op.defines for half in (LOW, HIGH)):
                    continue
                carried = op.merges
                read = widths(op)
                described = op.kind is not mir.Kind.OPAQUE and not op.barrier
                for one in op.uses:
                    if one in carried:
                        # Read for the half it is merged into, and only if
                        # something reads that half of the result.
                        if (carried[one], HIGH) in out:
                            out.add((one, HIGH))
                        if one not in read:
                            continue
                    if not described or one not in read:
                        # Nothing written down says how much of it is read.
                        out.add((one, LOW))
                        out.add((one, HIGH))
                        continue
                    out.add((one, LOW))
                    if read[one] >= 4:
                        out.add((one, HIGH))
                for ref in op.loads + op.stores:
                    for one in (ref.base, ref.segment):
                        if one is not None:
                            out.add((one, LOW))
                            if one == ref.segment or ref.base_width >= 4:
                                out.add((one, HIGH))
            for phi in block.phis:
                for half in (LOW, HIGH):
                    if (phi.result, half) in out:
                        out |= {(one, half) for one in phi.incoming.values()}
        changing = len(out) != before
    return out


def live(body: MirBody) -> set:
    """Values some half of which something reads.

    halves() is the analysis; this is the projection every caller that only
    asks "is this value read at all" wants.
    """
    return {one for one, _ in halves(body)}


# Whether a comparison holds. Keyed on what the branch tests, which the
# raise decided; keyed on the branch's mnemonic this was a pass that had to
# know x86 spells "less than" six different ways.
_TAKEN = {
    mir.Kind.EQ: lambda a, b, u: a == b,
    mir.Kind.NE: lambda a, b, u: a != b,
    mir.Kind.LT: lambda a, b, u: a < b,
    mir.Kind.LE: lambda a, b, u: a <= b,
    mir.Kind.GT: lambda a, b, u: a > b,
    mir.Kind.GE: lambda a, b, u: a >= b,
    mir.Kind.BELOW: lambda a, b, u: u(a) < u(b),
    mir.Kind.BELOW_EQ: lambda a, b, u: u(a) <= u(b),
    mir.Kind.ABOVE: lambda a, b, u: u(a) > u(b),
    mir.Kind.ABOVE_EQ: lambda a, b, u: u(a) >= u(b),
}


def _signed(fact) -> int:
    """A Known as the number the machine would compare."""
    top = 1 << (fact.width * 8 - 1)
    return fact.n - (top << 1) if fact.n & top else fact.n


def _comparison(block, op: Op):
    """The modeled comparison supplying this branch's condition value."""
    if op.kind is not mir.Kind.BRANCH or op.test not in _TAKEN:
        return None
    reads = [one for one in op.uses if one.flags]
    if len(reads) != 1:
        return None

    # The comparison this branch reads, which must be the last thing to
    # write the flags before it -- SSA says so by naming the value.
    where = next(
        ((index, one) for index, one in enumerate(block.ops) if reads[0] in one.defines),
        None,
    )
    if where is None:
        return None
    index, compare = where
    # A comparison in MIR's own terms: it subtracts and keeps only the
    # flags, so its two operands are its args. Asking the instruction meant
    # matching ir.Operation.COMPARE and reading ir.Loc operands out of it.
    if compare.kind is mir.Kind.SUB and len(compare.args) == 2 and not compare.results:
        return index, compare
    if (
        compare.kind in (mir.Kind.AND, mir.Kind.OR, mir.Kind.XOR)
        and op.test in (mir.Kind.EQ, mir.Kind.NE)
        and not compare.barrier
        and len(compare.args) == 2
        and len(compare.results) == 1
        and isinstance(result := compare.results[0], mir.Held)
        and result.width in (2, 4, 8)
        and all(isinstance(arg, (mir.Held, mir.Const)) and arg.width == result.width for arg in compare.args)
    ):
        return index, compare
    return None


def _outcome(block, op: Op, facts: dict, held: dict, pointers=None) -> bool | None:
    """Whether this branch is taken, where both its operands are numbers."""
    comparison = _comparison(block, op)
    if comparison is None:
        return None
    index, compare = comparison
    if compare.kind is not mir.Kind.SUB:
        result = consts._result(compare, facts)
        if result is None or result.width < compare.results[0].width:
            return None
        zero = consts.masked(result.n, compare.results[0].width) == 0
        return zero if op.test is mir.Kind.EQ else not zero
    parts = [consts._operand(compare, one, facts, held.get((block.at, index))) for one in compare.args]
    if any(one is None for one in parts):
        if op.test not in (mir.Kind.EQ, mir.Kind.NE) or pointers is None:
            return None
        pointer = next(
            (
                arg.value
                for arg, other in zip(compare.args, reversed(compare.args), strict=True)
                if isinstance(arg, mir.Held)
                and isinstance(other, mir.Const)
                and consts.masked(other.n, other.width) == 0
            ),
            None,
        )
        if pointer is None or not pointers.nonnull(pointer):
            return None
        return op.test is mir.Kind.NE
    left, right = parts
    width = max(left.width, right.width)
    return _TAKEN[op.test](_signed(left), _signed(right), lambda n: consts.masked(n, width))


def _switch_target(op: mir.Op, facts: dict[mir.Value, consts.Known]) -> int | None:
    if op.kind is not mir.Kind.SWITCH or len(op.args) != 1 or op.target is None:
        return None
    if (
        not isinstance(op.args[0], (mir.Held, mir.Const))
        or op.args[0].width not in (1, 2, 4)
        or op.defines
        or op.results
        or op.loads
        or op.stores
        or op.merges
        or op.barrier
        or op.stack is not None
        or op.floating is not None
    ):
        return None
    cases = [consts.masked(number, op.args[0].width) for number, _ in op.cases]
    if len(set(cases)) != len(cases):
        return None
    value = consts._operand(op, op.args[0], facts)
    if value is None:
        return None
    return next((target for number, target in op.cases if consts.masked(number, value.width) == value.n), op.target)


def _executable_successors(block, facts, states, held, pointers=None):
    from qbopt.analysis.constant_cycles import State

    if not block.ops:
        return block.succ
    last = block.ops[-1]
    if last.kind is mir.Kind.SWITCH:
        target = _switch_target(last, facts)
        if target in block.succ:
            return (target,)
        if any(isinstance(arg, mir.Held) and states.get(arg.value) is State.PENDING for arg in last.args):
            return None
        return block.succ
    if len(block.succ) != 2:
        return block.succ
    if last.target not in block.succ:
        return block.succ
    answer = _outcome(block, last, facts, held, pointers)
    if answer is not None:
        return (last.target,) if answer else tuple(at for at in block.succ if at != last.target)
    comparison = _comparison(block, last)
    if comparison is not None:
        _, compare = comparison
        if any(isinstance(arg, mir.Held) and states.get(arg.value) is State.PENDING for arg in compare.args):
            return None
    return block.succ


def _threaded(body: MirBody) -> MirBody:
    """Bypass empty control-flow blocks without changing any incoming phi value."""
    known = {block.at: block for block in body.blocks}
    predecessors = loopy.predecessors(body.blocks)
    loop_edges = set()
    for loop in loopy.loops(body.blocks, body.entry):
        outside = predecessors[loop.header] - loop.body
        if len(outside) == 1:
            parent = next(iter(outside))
            if known[parent].succ == (loop.header,):
                loop_edges.add(parent)
        if len(loop.latches) == 1:
            parent = next(iter(loop.latches))
            if known[parent].succ == (loop.header,):
                loop_edges.add(parent)
        loop_edges.update(
            block.at
            for block in body.blocks
            if block.at not in loop.body
            and predecessors[block.at]
            and predecessors[block.at] <= loop.body
            and len(block.succ) == 1
            and block.succ[0] not in loop.body
        )
    redirects = {}
    explicit_jumps = set()
    for block in body.blocks:
        # Loop-simplify form deliberately keeps a unique entry edge and a
        # unique backedge and dedicated exits as blocks of their own. Threading
        # one may be locally valid, but recreates the mixed entry, latch or
        # exit shape LoopSimplify has to rebuild on the next fixed-point round.
        if block.phis or len(block.succ) != 1 or block.at in loop_edges:
            continue
        ops = block.ops
        if ops and ops[-1].kind is mir.Kind.JUMP and ops[-1].target == block.succ[0]:
            explicit_jumps.add(block.at)
            ops = ops[:-1]
        if any(
            op.kind is not mir.Kind.NOTHING
            or op.defines
            or op.uses
            or op.loads
            or op.stores
            or op.args
            or op.results
            or op.merges
            or op.barrier
            or op.floating is not None
            or op.stack is not None
            for op in ops
        ):
            continue
        successor = known.get(block.succ[0])
        if successor is not None and not successor.phis:
            redirects[block.at] = successor.at

    def destination(start, source, *, implicit=False):
        target, seen = start, {source}
        while target in redirects and target not in seen:
            if implicit and target in explicit_jumps:
                break
            seen.add(target)
            target = redirects[target]
        return start if target in seen else target

    blocks = []
    changed = False
    for block in body.blocks:
        last = block.ops[-1] if block.ops else None
        if last is not None and last.kind is mir.Kind.SWITCH:
            blocks.append(block)
            continue
        explicit = last.target if last is not None and last.kind in {mir.Kind.JUMP, mir.Kind.BRANCH} else None
        successors = tuple(dict.fromkeys(destination(at, block.at, implicit=at != explicit) for at in block.succ))
        if successors == block.succ:
            blocks.append(block)
            continue
        changed = True
        ops = block.ops
        if ops and ops[-1].kind in {mir.Kind.JUMP, mir.Kind.BRANCH}:
            last = ops[-1]
            last = replace(last, target=destination(last.target, block.at))
            if last.kind is mir.Kind.BRANCH and len(successors) == 1:
                last = replace(
                    last,
                    op=ir.Operation.JUMP,
                    kind=mir.Kind.JUMP,
                    name="jmp",
                    uses=(),
                    args=(),
                    results=(),
                    test=None,
                    target=successors[0],
                )
            ops = (*ops[:-1], last)
        blocks.append(replace(block, ops=ops, succ=successors))

    # If both arms reach the same block through otherwise empty jump
    # trampolines, the condition has no semantic successor to choose.  Keep a
    # real jump at the source: the implicit arm may need one when source bodies
    # are interleaved, which is why ordinary threading above deliberately does
    # not erase that trampoline.
    converged = []
    for block in blocks:
        last = block.ops[-1] if block.ops else None
        if last is None or last.kind is not mir.Kind.BRANCH or len(block.succ) != 2:
            converged.append(block)
            continue
        destinations = tuple(destination(at, block.at) for at in block.succ)
        if len(set(destinations)) != 1:
            converged.append(block)
            continue
        target = destinations[0]
        jump = replace(
            last,
            op=ir.Operation.JUMP,
            kind=mir.Kind.JUMP,
            name="jmp",
            uses=(),
            args=(),
            results=(),
            test=None,
            target=target,
        )
        converged.append(replace(block, ops=(*block.ops[:-1], jump), succ=(target,)))
        changed = True
    return _unreachable(replace(body, blocks=tuple(converged))) if changed else body


def decided(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """A branch on two numbers, resolved.

    `IF a < b` with both constants is decided where it stands, and leaving
    it to run costs the comparison, the jump, and everything on the arm
    that cannot be taken. bools is three of them over four constants and
    nothing else.

    Taken becomes an unconditional jump and not-taken becomes an inert owner.
    What becomes unreachable is dropped by resolving the body afterwards
    rather than here.
    """
    body = _threaded(body)
    facts = consts.known(body, dgroup, calls)
    held = consts.cells(body, dgroup, calls, facts)
    from qbopt.analysis import alias

    pointers = alias.points_to(body)
    from qbopt.analysis import constant_cycles

    facts = constant_cycles.propagated(
        body, facts, lambda block, values, states: _executable_successors(block, values, states, held, pointers)
    )
    from qbopt.analysis import ranges

    scoped = ranges.bounded(body)

    out = []
    changed = False
    for block in body.blocks:
        if not block.ops:
            out.append(block)
            continue
        last = block.ops[-1]
        if last.kind is mir.Kind.SWITCH:
            target = _switch_target(last, facts)
            if target not in block.succ:
                out.append(block)
                continue
            jump = replace(
                last,
                kind=mir.Kind.JUMP,
                target=target,
                cases=(),
                args=(),
                uses=(),
                defines=(),
                results=(),
                name="",
                raised=None,
            )
            out.append(replace(block, ops=(*block.ops[:-1], jump), succ=(target,)))
            changed = True
            continue
        answer = _outcome(block, last, facts, held, pointers)
        if answer is None and block.at in scoped and last.kind is mir.Kind.BRANCH and len(block.succ) == 2:
            possible = tuple(at for at in block.succ if ranges.on_edge(block, at, scoped[block.at], facts) is not None)
            if len(possible) == 1:
                answer = possible[0] == last.target
        if answer is None:
            out.append(block)
            continue
        target = last.target
        if target is None or target not in {one.at for one in body.blocks}:
            out.append(block)
            continue
        changed = True
        if answer:
            jump = replace(
                last,
                kind=mir.Kind.JUMP,
                uses=(),
                args=(),
                results=(),
                target=target,
            )
            out.append(replace(block, ops=block.ops[:-1] + (jump,), succ=(target,)))
        else:
            kept = _absorb(list(block.ops), {last.at})
            if kept == list(block.ops):
                out.append(block)
                continue
            out.append(replace(block, ops=tuple(kept), succ=tuple(at for at in block.succ if at != target)))
    if not changed:
        return body
    # Remove dead edges without losing the unreachable blocks' byte ownership.
    return _trivial_phis(_unreachable(replace(body, blocks=tuple(out))))


def _unreachable(body: MirBody) -> MirBody:
    """Dead blocks retain byte ownership, but no instructions or outgoing edges."""
    blocks = {block.at: block for block in body.blocks}
    reached, pending = set(), [body.entry]
    while pending:
        at = pending.pop()
        if at in reached or at not in blocks:
            continue
        reached.add(at)
        pending.extend(blocks[at].succ)
    return replace(
        body,
        blocks=tuple(
            normalized
            for block in body.blocks
            if block.at in reached or block.ops
            for normalized in (
                block
                if block.at in reached
                else replace(
                    block,
                    succ=(),
                    phis=(),
                    # One definition of an inert source owner.  The former
                    # partial spelling forgot floating semantics (and several
                    # other semantic fields), producing a NOTHING operation
                    # that lowering correctly refused after exact loop peeling
                    # made an x87 residual body unreachable.
                    ops=tuple(_empty_operation(op) for op in block.ops),
                ),
            )
        ),
    )


def _trivial_phis(body: MirBody) -> MirBody:
    """Resolve single-valued joins after an edge disappears, without discarding byte ownership."""
    predecessors = loopy.predecessors(body.blocks)
    swaps = {}
    while True:
        changed = False
        out = []
        for block in body.blocks:
            phis = []
            for phi in block.phis:
                incoming = {
                    at: _provider(value, swaps) for at, value in phi.incoming.items() if at in predecessors[block.at]
                }
                values = set(incoming.values()) - {phi.result}
                if len(values) == 1:
                    swaps[phi.result.id] = next(iter(values))
                    changed = True
                else:
                    phis.append(mir.Phi(phi.result, incoming))
            out.append(replace(block, phis=tuple(phis), ops=tuple(_substituted(op, swaps) for op in block.ops)))
        body = replace(body, blocks=tuple(out))
        if not changed:
            return body


def dead(body: MirBody) -> MirBody:
    """Operations whose results nothing reads, removed.

    BC emits no dead code -- 7 operations in the whole corpus before any
    pass runs -- so this is for what the passes leave behind: folding turns
    a load into a move of a number and what fed it stops being read, and
    hoisting takes a computation out and leaves the copy standing in for it.

    live() is the analysis, and what it needs is that every use list is
    complete. For a call that means its contract naming the registers it
    reads, which is runtime.Contract.inputs and was written for this.
    A removed computation leaves an empty ownership marker. Its disjoint
    input ranges stay accounted for without requiring an adjacent survivor
    or keeping a dead value live through allocation.
    """
    # Incomplete readers forbid global removal, but a result overwritten
    # locally before reaching one cannot supply its hidden inputs.
    limited = any(
        (one.barrier or one.kind is mir.Kind.OPAQUE) and not one.reads_complete
        for block in body.blocks
        for one in block.ops
    )
    if not limited:
        body = _pruned_phis(body, live(body))
    alive = live(body)
    if limited:
        for block in body.blocks:
            overwritten = _overwritten_locally(block)
            alive.update(value for op in block.ops for value in op.defines if value not in overwritten)
            alive.update(value for phi in block.phis for value in phi.incoming.values())
        while True:
            before = len(alive)
            alive.update(
                value
                for block in body.blocks
                for op in block.ops
                if any(value in alive for value in op.defines)
                for value in op.uses
            )
            if len(alive) == before:
                break
    out = []
    changed = False
    for block in body.blocks:
        # Several semantic operations may share an input address. Their
        # computations are independent even when their provenance is not.
        overwritten = _overwritten_locally(block) if limited else None
        gone = {
            id(op)
            for op in block.ops
            if _removable(op, alive) and (overwritten is None or set(op.defines) <= overwritten)
        }
        if not gone:
            out.append(block)
            continue
        ops = [_empty_operation(op) if id(op) in gone else op for op in block.ops]
        changed = True
        out.append(replace(block, ops=tuple(ops)))
    if not changed:
        return body
    removed = {value for block in body.blocks for op in block.ops for value in op.defines} - {
        value for block in out for op in block.ops for value in op.defines
    }
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    replace(
                        op,
                        uses=tuple(value for value in op.uses if value not in removed or value not in op.merges),
                        merges={before: after for before, after in op.merges.items() if before not in removed},
                    )
                    for op in block.ops
                ),
            )
            for block in out
        ),
    )


def _overwritten_locally(block) -> set:
    """Results replaced before reaching an opaque reader or a block exit."""
    written = set()
    overwritten = set()
    for op in reversed(block.ops):
        if op.barrier or op.kind is mir.Kind.OPAQUE:
            written.clear()
            continue
        overwritten.update(value for value in op.defines if value.version and (value.variable, value.flags) in written)
        written.update((value.variable, value.flags) for value in op.defines if value.version)
    return overwritten


def _kept(op: Op) -> bool:
    """Whether this operation stays whatever the liveness says.

    `halves` and `_removable` have to agree on this, or `dead` deletes what
    a kept operation reads: a `cmp` whose flags nothing reads any more is
    still emitted -- it defines nothing else -- and would go on comparing a
    value whose definition went.
    """
    if op.kind in _OBSERVED or op.stores or op.barrier:
        return True
    if op.kind is mir.Kind.OPAQUE:
        return True
    if op.kind is mir.Kind.SUB and not op.results and op.defines:
        return False
    return not [one for one in op.defines if not one.flags]


def _removable(op: Op, alive: set) -> bool:
    """Whether anything at all would notice this operation going."""
    if _kept(op):
        return False
    return not any(one in alive for one in op.defines)


def _folded_division(op: Op, numbers: tuple[int, int], wanted: set) -> tuple[Op, ...]:
    results = {result.value for result in op.results}
    if any(value in wanted and value not in results for value in op.defines):
        return (op,)
    return tuple(
        replace(
            op,
            op=ir.Operation.MOVE,
            name="mov",
            kind=mir.Kind.COPY,
            args=(mir.Const(number, 4),),
            results=(result,),
            defines=(result.value,),
            uses=(),
            loads=(),
            merges={},
            raised=None,
            symbol=False,
            source_backed=False,
            id=op.id if index == 0 else None,
            absorbed=op.absorbed if index == 0 else (),
        )
        for index, (result, number) in enumerate(zip(op.results, numbers, strict=True))
    )


_EDGE_FOLDABLE = _PURE - frozenset(
    {
        # A copy removes no computation.  Addresses and pointer offsets carry
        # provenance, while division and remainder may trap.  None is a pure
        # integer expression that this first, deliberately strict form may
        # speculate separately on incoming edges.
        mir.Kind.COPY,
        mir.Kind.ADDRESS,
        mir.Kind.PTR_OFFSET,
        mir.Kind.DIV,
        mir.Kind.REM,
    }
)


def _folded_phi_edges(body: MirBody, facts: dict, wanted: set) -> MirBody:
    """Fold one pure join expression independently on every incoming edge.

    A phi is not itself constant when its arms differ, so ordinary SCCP quite
    correctly leaves ``phi(7, 9) + 1`` alone.  Each use is nevertheless a
    constant on the edge which supplies it.  Translate the expression through
    every complete phi, evaluate it there, and join the resulting constants.

    Only unconditional edges are used.  Splitting a conditional edge belongs
    to CFG construction, and moving observable or trapping work there would
    change the program.  One expression is handled per call so the ordinary
    MIR fixed point and dead-code pass expose and clean up each subsequent
    opportunity.
    """
    predecessors = loopy.predecessors(body.blocks)
    by_at = {block.at: block for block in body.blocks}
    pointer_values = set(body.pointer_values)
    values = tuple(_ssa_values(body))
    serial = max((value.id for value in values), default=0) + 1
    variable = max((value.variable for value in values), default=0) + 1

    for block in body.blocks:
        parents = predecessors.get(block.at, frozenset())
        if len(parents) < 2 or not block.phis:
            continue
        parent_blocks = {at: by_at.get(at) for at in parents}
        if any(parent is None or parent.succ != (block.at,) for parent in parent_blocks.values()):
            continue
        # A terminal conditional with one surviving CFG successor still has
        # path semantics which are not represented by that tuple alone.
        if any(
            parent.ops and parent.ops[-1].kind in (mir.Kind.BRANCH, mir.Kind.SWITCH, mir.Kind.RETURN)
            for parent in parent_blocks.values()
        ):
            continue
        phis = {phi.result: phi for phi in block.phis if set(phi.incoming) == set(parents)}
        if not phis:
            continue

        corridor = [block]
        seen = {block.at}
        while len(corridor[-1].succ) == 1:
            successor = by_at.get(corridor[-1].succ[0])
            if (
                successor is None
                or successor.at in seen
                or predecessors.get(successor.at, frozenset()) != frozenset({corridor[-1].at})
            ):
                break
            corridor.append(successor)
            seen.add(successor.at)

        for operation_block in corridor:
            for index, op in enumerate(operation_block.ops):
                if (
                    op.kind not in _EDGE_FOLDABLE
                    or not mir.instruction(op)
                    or op.barrier
                    or op.loads
                    or op.stores
                    or op.floating is not None
                    or op.stack is not None
                    or op.merges
                    or op.opaque_defs != frozenset()
                    or op.opaque_uses != frozenset()
                    or mir.partial(op)
                ):
                    continue
                target = consts._defined(op)
                results = [result for result in op.results if isinstance(result, mir.Held)]
                if (
                    target is None
                    or len(results) != 1
                    or results[0].value != target
                    or target in pointer_values
                    or any(value != target and value in wanted for value in op.defines)
                ):
                    continue
                used = {arg.value for arg in op.args if isinstance(arg, mir.Held)}
                if not used.intersection(phis):
                    continue

                width = results[0].width
                numbers: dict[int, int] = {}
                for parent in parents:
                    swap = {result.id: phi.incoming[parent] for result, phi in phis.items()}
                    fact = consts._result(_substituted(op, swap), facts)
                    if fact is None or fact.width < width:
                        break
                    numbers[parent] = consts.masked(fact.n, width)
                if len(numbers) != len(parents):
                    continue

                changed = dict(by_at)
                incoming: dict[int, mir.Value] = {}
                for offset, parent_at in enumerate(sorted(parents)):
                    parent = parent_blocks[parent_at]
                    edge_value = mir.Value(serial + offset, parent.at, variable=variable + offset, version=1)
                    incoming[parent_at] = edge_value
                    copy = mir.Op(
                        parent.at,
                        ir.Operation.MOVE,
                        "mov",
                        (edge_value,),
                        (),
                        kind=mir.Kind.COPY,
                        args=(mir.Const(numbers[parent_at], width),),
                        results=(mir.Held(edge_value, width),),
                        source_backed=False,
                    )
                    ops = list(parent.ops)
                    position = len(ops) - 1 if ops and ops[-1].kind is mir.Kind.JUMP else len(ops)
                    ops.insert(position, copy)
                    changed[parent_at] = replace(parent, ops=tuple(ops))

                join = changed[block.at]
                changed[block.at] = replace(join, phis=(*join.phis, mir.Phi(target, incoming)))
                owner = changed[operation_block.at]
                ops = list(owner.ops)
                ops[index] = _empty_operation(op)
                changed[operation_block.at] = replace(owner, ops=tuple(ops))
                return replace(body, blocks=tuple(changed[one.at] for one in body.blocks))
    return body


def folded(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """An operation whose result is a number, replaced by that number.

    `n = 7 : k = 3` and then `n * k` inside a loop is three stores and two
    loads and a multiply, all of it computing 21 on every pass. consts.known
    says which values are numbers -- reading through memory, because BC
    keeps every variable there and nothing else would reach past the first
    store -- and this writes them down.

    SSA in, SSA out: the operation goes on defining the value it defined,
    and simply computes it from nothing. What it read it no longer reads,
    so its uses and loads go, which is what lets dead code elimination see
    the loads afterwards.

    Refused where the flags it also sets are read. `add ax,[c]` computes a
    number and a carry, and only one of them is expressible as `mov ax,n`.
    """
    from qbopt.analysis import floatfacts

    edges = floatfacts.exit_cells(body, dgroup, calls)
    facts = consts.known(body, dgroup, calls, edges=edges)
    floating_facts = (
        floatfacts.known(body, dgroup, calls) if any(op.floating for block in body.blocks for op in block.ops) else {}
    )
    conversions = floatfacts.converted(body, dgroup, calls, facts=floating_facts)
    argument_facts = facts | conversions
    memory = (
        consts.cells(body, dgroup, calls, facts, edges=edges)
        if any(op.loads or op.kind is mir.Kind.DIVMOD for block in body.blocks for op in block.ops)
        else {}
    )
    symbols = _symbol_copies(body)
    if not facts and not memory and not argument_facts and not symbols:
        return body

    # Live, not merely mentioned: see live()'s own note on hotlop's dx.
    wanted = live(body)

    out = []
    changed = False
    for block in body.blocks:
        ops = []
        for index, op in enumerate(block.ops):
            numbers = consts.division(op, facts, memory.get((block.at, index), {}))
            if numbers is not None:
                replacements = _folded_division(op, numbers, wanted)
                ops.extend(replacements)
                changed |= replacements != (op,)
                continue
            updated = _constant_update(op, facts, memory.get((block.at, index), {}), wanted)
            made = _constant_operands(
                _folded_op(updated, facts, wanted),
                argument_facts if op.kind is mir.Kind.ARG else facts,
                memory.get((block.at, index), {}),
                symbols,
            )
            changed = changed or made is not op
            ops.append(made)
        out.append(replace(block, ops=tuple(ops)))
    from qbopt.optimize import floatfold

    result = replace(body, blocks=tuple(out)) if changed else body
    result = _folded_phi_edges(result, facts, wanted)
    # An exact exit fact describes only the path leaving a numeric loop.  It
    # may fold a successor load, but it is not permission for ordinary
    # constant folding to replace the loop's strict x87 operations and their
    # observation points with stores.  That transformation belongs to the
    # dedicated FP loop specialization, which retains its final checked
    # iteration.  Straight-line FP work remains eligible for the existing
    # storage/conversion folds.
    if loopy.loops(result.blocks, result.entry):
        return result
    return floatfold.stored(floatfold.discarded(result, conversions), floating_facts)


def _constant_update(op: Op, facts: dict, memory: dict, wanted: set) -> Op:
    if any(value in wanted for value in op.defines):
        return op
    fact = consts.updated(op, facts, memory)
    if fact is None:
        return op
    address_values = {value for ref in op.stores for value in (ref.base, ref.segment) if value is not None}
    return replace(
        op,
        op=ir.Operation.MOVE,
        kind=mir.Kind.STORE,
        name="mov",
        defines=(),
        uses=tuple(value for value in op.uses if value in address_values),
        loads=(),
        args=(mir.Const(fact.n, fact.width),),
        source_backed=False,
        raised=None,
        symbol=False,
    )


def _symbol_copies(body: MirBody) -> dict:
    """Values that are a symbol's address, with the op that owns its fixup.

    Unlike a number, a symbol is not completely described by its literal
    value: the defining operation's identity leads emission back to the OMF
    fixup, including its frame.  A consumer that substitutes the symbol must
    take that identity with it.
    """
    return {
        op.defines[0]: (op.args[0], op)
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.COPY
        and len(op.args) == len(op.defines) == 1
        and isinstance(op.args[0], mir.Symbol)
        and not (op.loads or op.merges)
    }


def _literal_of(arg, facts: dict, symbols: dict) -> "mir.Const | mir.Symbol | None":
    """A register operand as the literal it holds, a number or a symbol's address."""
    if not isinstance(arg, mir.Held):
        return None
    fact = facts.get(arg.value)
    if fact is not None and fact.width >= arg.width:
        return mir.Const(consts.masked(fact.n, arg.width), arg.width)
    known = symbols.get(arg.value)
    symbol = known[0] if known is not None else None
    return symbol if symbol is not None and symbol.width == arg.width else None


def _constant_operands(op: Op, facts: dict, memory: dict | None = None, symbols: dict | None = None) -> Op:
    """Propagate width-proven constants without reversing ordered operands."""
    if op.kind is mir.Kind.ARG:
        return _constant_argument(op, facts, memory or {}, symbols or {})
    if (
        op.kind is mir.Kind.STORE
        and len(op.args) == len(op.stores) == 1
        and not op.defines
        and not op.merges
        and not op.loads
        and not op.barrier
        and op.floating is None
        and isinstance(arg := op.args[0], mir.Held)
        and arg.width == op.stores[0].width
        and (literal := _literal_of(arg, facts, symbols or {})) is not None
    ):
        address_values = {value for ref in op.stores for value in (ref.base, ref.segment) if value is not None}
        # The node and its field stay: the destination is still this op's,
        # and segld's `mov [x],ax` folded to `mov [x],6` counted its fixup
        # as gone while emitting a new one.
        return replace(
            op,
            args=(literal,),
            uses=tuple(value for value in op.uses if value != arg.value or value in address_values),
            raised=None,
        )
    if (
        op.kind
        not in (
            mir.Kind.ADD,
            mir.Kind.ADD_CARRY,
            mir.Kind.AND,
            mir.Kind.OR,
            mir.Kind.XOR,
            mir.Kind.MUL,
            mir.Kind.SUB,
            mir.Kind.SUB_BORROW,
            mir.Kind.DIVMOD,
            mir.Kind.PTR_OFFSET,
        )
        or len(op.args) != 2
    ):
        return op
    if op.kind is mir.Kind.MUL and len(op.results) != 1:
        return op
    replaced = set()
    removed = set()
    args = []
    ordered = op.kind in (mir.Kind.SUB, mir.Kind.SUB_BORROW, mir.Kind.DIVMOD, mir.Kind.PTR_OFFSET)
    for index, arg in enumerate(op.args):
        if (
            (not ordered or index == 1)
            and isinstance(arg, mir.Held)
            and (fact := facts.get(arg.value)) is not None
            and fact.width >= arg.width
        ):
            args.append(mir.Const(consts.masked(fact.n, arg.width), arg.width))
            replaced.add(arg.value)
        elif (
            (not ordered or index == 1)
            and isinstance(arg, mir.Cell)
            and not op.stores
            and not op.barrier
            and arg.ref in op.loads
            and (fact := consts._cell(memory or {}, arg.ref)) is not None
        ):
            args.append(mir.Const(fact.n, arg.ref.width))
            removed.add(arg.ref)
        else:
            args.append(arg)
    if not replaced and not removed:
        return op
    if not ordered and isinstance(args[0], mir.Const) and isinstance(args[1], mir.Held):
        args.reverse()
    retained = {arg.value for arg in args if isinstance(arg, mir.Held)}
    retained.update(value for ref in op.loads + op.stores for value in (ref.base, ref.segment) if value is not None)
    return replace(
        op,
        args=tuple(args),
        loads=tuple(ref for ref in op.loads if ref not in removed),
        source_backed=False if removed else op.source_backed,
        raised=None if removed else op.raised,
        uses=tuple(value for value in op.uses if value not in replaced or value in op.merges or value in retained),
    )


def _constant_argument(op: Op, facts: dict, memory: dict, symbols: dict | None = None) -> Op:
    """Substitute the value read for an argument, keeping its stack write."""
    if len(op.args) != 1 or op.defines or op.merges or op.barrier:
        return op
    arg = op.args[0]
    if not isinstance(arg, (mir.Held, mir.Cell)):
        return op
    if isinstance(arg, mir.Held) and op.loads:
        return op
    if isinstance(arg, mir.Cell) and (op.loads != (arg.ref,) or arg.ref in op.stores):
        return op
    width = arg.ref.width if isinstance(arg, mir.Cell) else arg.width
    owner = None
    fact = consts._operand(op, arg, facts, memory)
    if fact is not None and fact.width >= width:
        literal = mir.Const(consts.masked(fact.n, width), width)
    else:
        known = (symbols or {}).get(arg.value) if isinstance(arg, mir.Held) else None
        if known is None or known[0].width != width:
            return op
        literal, owner = known
    kept = tuple(ref for ref in op.loads if not isinstance(arg, mir.Cell) or ref != arg.ref)
    uses = tuple(
        dict.fromkeys(value for ref in (*kept, *op.stores) for value in (ref.base, ref.segment) if value is not None)
    )
    return replace(
        op,
        args=(literal,),
        uses=uses,
        loads=kept,
        source_backed=False,
        raised=None,
        # A number owns no relocation.  A symbol takes the defining copy's
        # identity as well as its value: that is how emission moves the
        # original fixup, including its frame, onto this argument.  Keeping
        # the argument's id instead emitted `push 0`; inventing a fresh
        # fixup instead mistook DIVMOD's CS-relative handler for DGROUP data.
        id=owner.id if owner is not None else op.id,
        symbol=owner is not None,
    )


def _folded_op(op: Op, facts: dict, wanted: set) -> Op:
    """The operation as a move of its own answer, where that is possible."""
    if not mir.instruction(op) or op.stores:
        return op
    # Not a register move. Rewriting `mov ax,cx` to `mov ax,3` removes no
    # work -- the same instruction, the same length, nothing read that was
    # not already in a register -- and it undoes an allocation: a live range
    # split is exactly that move, so folding it puts the computation back
    # inside the loop the hoist took it out of, and the hoist lifts it again
    # next round. Three bytes a round, for ever.
    #
    # What folding is for is removing a computation. A copy is not one.
    if op.kind is mir.Kind.COPY:
        return op
    if op.kind in (mir.Kind.JUMP, mir.Kind.BRANCH, mir.Kind.CALL, mir.Kind.RETURN):
        return op
    # It has to write a value. A widening multiply writes two -- and its
    # answer is the first, which the operation says itself.
    if not op.results or not isinstance(op.results[0], mir.Held):
        return op

    target = consts._defined(op)
    if target is None or target not in facts:
        return op
    # A second result that something reads is not expressible as one move.
    if any(one != target and one in wanted for one in op.defines):
        return op

    fact = facts[target]
    into = op.results[0]
    if fact.width < into.width:
        return op
    if op.kind is mir.Kind.COPY and any(isinstance(one, mir.Const) for one in op.args):
        return op  # already says so

    # In MIR's own operands: this value, that constant. What register it
    # ends up in is the allocator's, and lower.py is where it becomes one.
    return replace(
        op,
        kind=mir.Kind.COPY,
        defines=(target,),
        uses=(),
        loads=(),
        args=(mir.Const(fact.n, into.width),),
        results=(mir.Held(target, into.width),),
        symbol=False,
        source_backed=False,
        raised=None,
    )


def _placement(block, run: list, alive) -> int | None:
    """The latest place in the preheader every value the run reads is defined.

    Latest, because that is the shortest the result has to stay live. This
    is the whole of the question. Where the result then lives is the
    allocator's, and a body it cannot colour is laid out as it was raised
    -- which is what makes this enough.

    It used to be half the question: `_insertion` also picked a register
    out of target.AVAILABLE, `_reads_from` rewrote every consumer to name
    it, `_move` emitted the copies, `_writes_to` re-seated the destination
    and `_can_reseat` asked lir whether it could. That is register
    allocation in a MIR pass with none of the allocator's information, and
    it is where every hoist bug this year came from.
    """
    made = {value for one in run for value in one.defines}
    wants = {value for one in run for value in _consumed(one) if not value.flags and value not in made}
    ready = set(alive.live_in.get(block.at, ()))
    index = 0
    for number, one in enumerate(block.ops):
        if wants <= ready:
            index = number
        ready |= set(one.defines)
    if wants <= ready:
        index = len(block.ops)
    return index if wants <= ready else None


def _reparented(body: MirBody, crossed: set) -> MirBody:
    """Every value in `crossed` made a variable of its own.

    A rename and nothing else: the value keeps its identity, its readers
    keep reading it, and only which variable it is a version of changes.
    That is what stops the SSA re-derivation joining it to whatever else
    lived in the same place, which is the whole of what the hoist needed a
    spare register for.
    """
    values = (*body.values, *(value for block in body.blocks for op in block.ops for value in op.uses))
    taken = max((one.variable for one in values), default=0)
    instead: dict = {}
    for number, one in enumerate(sorted(crossed, key=lambda v: (v.variable, v.version)), 1):
        instead[one] = replace(one, variable=taken + number, version=1)

    def value(one):
        return instead.get(one, one)

    def arg(one):
        if isinstance(one, mir.Held) and one.value in instead:
            return mir.Held(instead[one.value], one.width)
        if isinstance(one, mir.Cell):
            return mir.Cell(cell(one.ref))
        return one

    def cell(one):
        base, segment = value(one.base) if one.base else one.base, value(one.segment) if one.segment else one.segment
        return one if (base is one.base and segment is one.segment) else replace(one, base=base, segment=segment)

    def op(one):
        return replace(
            one,
            defines=tuple(value(x) for x in one.defines),
            uses=tuple(value(x) for x in one.uses),
            args=tuple(arg(x) for x in one.args),
            results=tuple(arg(x) for x in one.results),
            loads=tuple(cell(x) for x in one.loads),
            stores=tuple(cell(x) for x in one.stores),
            merges={value(a): value(b) for a, b in one.merges.items()},
            raised=None
            if one.raised is None
            else (tuple(arg(x) for x in one.raised[0]), tuple(arg(x) for x in one.raised[1])),
        )

    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                phis=tuple(
                    replace(phi, result=value(phi.result), incoming={at: value(x) for at, x in phi.incoming.items()})
                    for phi in block.phis
                ),
                ops=tuple(op(one) for one in block.ops),
            )
            for block in body.blocks
        ),
        pointer_values=frozenset(value(one) for one in body.pointer_values),
        pointer_seeds={value(one): provenance for one, provenance in body.pointer_seeds.items()},
        integer_ranges={value(one): interval for one, interval in body.integer_ranges.items()},
    )


def _guaranteed_float_work(body: MirBody, loop: loopy.Loop, nonempty: bool) -> frozenset[int]:
    if not nonempty:
        return frozenset()
    blocks = {block.at: block for block in body.blocks}
    if any(not _unchanged_float_environment(op) for at in loop.body for op in blocks[at].ops):
        return frozenset()
    starts = [at for at in blocks[loop.header].succ if at in loop.body and at != loop.header]
    if len(starts) != 1:
        return frozenset()

    def reaches(at: int, target: int, visiting: frozenset[int]) -> bool:
        if at == target:
            return True
        if at == loop.header or at not in loop.body or at in visiting:
            return False
        successors = blocks[at].succ
        return bool(successors) and all(reaches(to, target, visiting | {at}) for to in successors)

    return frozenset(
        id(op)
        for at in loop.body
        if at == loop.header or reaches(starts[0], at, frozenset())
        for op in blocks[at].ops
        if op.floating is not None
    )


def _literal(op: Op) -> bool:
    """A constant that stayed a definition: fold puts every literal its reader
    can take into the reader, so what is left is work, like a selector."""
    return op.kind is mir.Kind.COPY and bool(op.args) and all(isinstance(arg, mir.Const) for arg in op.args)


def hoisted(body: MirBody, dgroup: frozenset[int], calls: dict[int, str], bounds: dict | None = None) -> MirBody:
    """A loop-invariant run of operations, done once before the loop.

    `mov ax,[n] / imul word [k]` computes the same product on every pass of
    a loop that writes neither. Moving the whole run rather than the load
    alone is what makes it possible at all: `imul`'s multiplicand is ax
    implicitly, so renaming the load's destination leaves the multiply
    reading a register nothing put anything in -- hotlop printed 0 for 630
    that way. With the run moved, the implicit registers are used inside it,
    in the preheader, and only its result has to survive into the loop.

    That result needs a register the loop does not touch, which is what
    `pins` asks regalloc for. Refused, the whole thing is dropped: a value
    hoisted into a register the loop clobbers is the wrong program.
    """
    inside = loopy.loops(list(body.blocks), body.entry)
    if not inside:
        return body
    from qbopt.analysis import ranges

    scoped = ranges.bounded(body)
    constant = ranges.constants(body)
    facts = {at: {**constant, **inside} for at, inside in scoped.items()}
    intervals = {id(op): facts.get(block.at, constant) for block in body.blocks for op in block.ops}
    at_of = {block.at: block for block in body.blocks}
    alive = alive_at.live(body)
    readable = live(body)
    effective = _effective(body, calls)
    crossed: set = set()
    demanded = halves(body)
    moved: dict[int, list[Op]] = {}
    gone: set[int] = set()
    placing: dict[int, int] = {}

    for loop in inside:
        into = _preheader(body, loop)
        if into is None or into in loop.body:
            continue
        ops = [one for at in sorted(loop.body) for one in at_of[at].ops]
        originals = ops
        ops = [
            replace(one, uses=tuple(value for value in one.uses if value not in one.merges), merges={})
            if one.kind is mir.Kind.COPY
            and len(one.args) == 1
            and isinstance(one.args[0], mir.Symbol)
            and all((value, HIGH) not in demanded for value in one.defines)
            else one
            for one in ops
        ]
        identities = {id(one): id(original) for one, original in zip(ops, originals, strict=True)}
        stores = [
            (ref, intervals.get(id(original)))
            for one, original in zip(ops, originals, strict=True)
            for ref in one.stores
        ]
        carried = {phi.result for at in loop.body for phi in at_of[at].phis}
        phis = [phi for at in loop.body for phi in at_of[at].phis]
        from qbopt.analysis import induction

        nonempty = induction.nonempty(body, loop)
        run = _invariant_run(
            ops,
            carried,
            stores,
            dgroup,
            calls,
            phis,
            bounds,
            _starts(phis),
            readable,
            intervals,
            nonempty=nonempty,
            floating_allowed=_guaranteed_float_work(body, loop, nonempty),
        )
        # Track operations, not source addresses: hoisted definitions share
        # their anchor's address with other computations and the jump. HARR
        # lost all of those when its descriptor moved a second time.
        run = [one for one in run if identities[id(one)] not in gone]
        while run:
            rest = [one for one in ops if one not in run]
            retained = {value for value in _crossed_values(run, rest, phis, effective) if value.flags}
            if not retained:
                break
            run = _pruned(run, retained)
        if not run:
            continue

        # What the run computes that the rest of the loop still reads. One
        # value, or this would need a register for each and a rule for
        # which; the shapes that pay have exactly one.
        rest = [one for one in ops if one not in run]
        crossing = _crossing(run, rest, phis, effective)
        if crossing is None:
            continue

        # Where it goes: the latest point every value it reads is defined.
        index = _placement(at_of[into], run, alive)
        if index is None:
            continue

        # Everything the run defines, not only what leaves the loop. The
        # run is a chain and its own intermediate values live in the
        # preheader too: hotlpx's multiplicand load is a version of the
        # same variable as the counter, so leaving it alone put its
        # definition after `mov ax,1` and the phi carried the load into the
        # loop as though it were the counter.
        crossed |= {value for one in run for value in one.defines if not value.flags}
        placing[into] = min(placing.get(into, index), index)
        moved[into] = moved.get(into, []) + list(run)
        gone.update(identities[id(one)] for one in run)

    if not gone:
        return body

    out = []
    for block in body.blocks:
        ops = [one for one in block.ops if id(one) not in gone]

        if ops and block.ops and id(block.ops[0]) in gone:
            # Onto the block's own address so branches still land, and told
            # what it stands for: `covers` otherwise means the bytes at
            # `at`, which are now the hoisted operation's, and both would
            # claim them.
            first = ops[0]
            ops = [replace(first, at=block.ops[0].at)] + ops[1:]
        if block.at in moved:
            leaves = bool(ops) and ops[-1].kind in (mir.Kind.JUMP, mir.Kind.BRANCH)
            lifted = list(moved[block.at])
            # Running in the preheader and still standing for the bytes it
            # came from, which is what a record or fixup naming those bytes
            # needs in order to follow it.
            # The address is what orders these once the body is emitted, so
            # a position in the list is not enough -- they have to take the
            # address of whatever they go in front of. `covers` is what says
            # which of BC's bytes they stand for, and is untouched.
            index = placing.get(block.at, len(ops))
            if leaves and index >= len(ops):
                index = max(0, len(ops) - 1)
            if ops:
                anchor = ops[index].at if index < len(ops) else ops[-1].at
                lifted = [replace(one, at=anchor) for one in lifted]
            ops = ops[:index] + lifted + ops[index:]
        out.append(replace(block, ops=tuple(ops)))

    # What crossed the loop edge is its own variable now.
    #
    # In MIR a register is a variable, and two values BC kept in one
    # register are one variable only while nothing has moved them. The
    # moment a computation leaves a loop they are not: hotlpx's product and
    # its counter both lived in ax, and re-deriving SSA put a phi over them
    # that said the loop's reads of the product were reads of the counter.
    # The allocator then saw no conflict, left both in ax, and the counter
    # reload destroyed the product on the second pass.
    #
    # This is what `_insertion` was doing by hand when it picked a spare
    # register: making the crossing value a different thing. It is a
    # statement about variables and says nothing about registers -- a fresh
    # variable has no origin, so the allocator places it wherever it likes,
    # which is its job.
    moved_out = replace(body, blocks=tuple(out))
    if crossed:
        moved_out = _reparented(moved_out, crossed)

    # Moving a definition to its dominating preheader preserves its SSA
    # edges. Reconstructing by variable number loses cross-variable phis
    # introduced by promotion and substitution, including accumulators.
    return moved_out


# The passes, in the order they run. One per whole-segment round, because
# each round re-raises the body from what the last one wrote -- an op's
# defines and uses are computed at raise time, so a pass that has already
# rewritten the op list is describing the body that went in, not the one
# that came out.
#
# Running them one at a time is what makes that ordering merely an order
# rather than a correctness argument. It was the latter: widening ran
# before avail.py once and avail forwarded a stale high half across an op
# that said it read two bytes where the instruction read four. That is no
# longer possible to get wrong by rearranging this list.


# Every transform, as the one thing a transform is. The functions above stay
# because they are what each class does and are what the tests name; what
# changes is that the pipeline can only reach them through `transform`.
class Fold(MIRTransform):
    name = "fold"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return folded(body, self.where.dgroup, self.where.named)


class Decide(MIRTransform):
    name = "decide"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        from qbopt.optimize import cfg

        return cfg.merged(decided(body, self.where.dgroup, self.where.named))


class Dead(MIRTransform):
    name = "dead"

    def transform(self, body: MirBody) -> MirBody:
        return dead(body)


class FloatLoop(MIRTransform):
    """Specialize a proven strict-FP recurrence before LICM changes its shape."""

    name = "floatloop"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        from qbopt.optimize import floatloop

        return floatloop.specialized(body, self.where.dgroup, self.where.calls)


class Hoist(MIRTransform):
    name = "hoist"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        body = hoisted(body, self.where.dgroup, self.where.named, self.where.bounds)
        return loopmotion.sunk_stores(body, self.where.dgroup, self.where.bounds, _handles_errors(self.where))


class DropStores(MIRTransform):
    name = "drop_stores"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        from qbopt.analysis import observers

        private = observers.private(body, self.where.found, self.where.blocks)
        return without_dead_stores(
            body, self.where.dgroup, self.where.named, private, self.where.bounds, _handles_errors(self.where)
        )


def _handles_errors(where: Where) -> bool:
    from qbopt.abi import runtime

    return runtime.handles_errors(map(runtime.contract, where.named.values()))


class Gvn(MIRTransform):
    """The single value-reuse pass: scalar GVN and memory-aware PRE."""

    name = "gvn"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        from qbopt.optimize import gvn

        return gvn.optimized(body, self.where)


class Place(MIRTransform):
    name = "place"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return placed(body, self.where.dgroup, self.where.named)


class Algebraic(MIRTransform):
    name = "algebraic"

    def transform(self, body: MirBody) -> MirBody:
        demanded = halves(body)
        return algebraic.simplified(
            body, {value for value, _ in demanded}, {value for value, part in demanded if part == HIGH}
        )


class SplitPointers(MIRTransform):
    """Expose packed far addresses as independently allocatable MIR values."""

    name = "split_pointers"

    def transform(self, body: MirBody) -> MirBody:
        return pointeraccess.split(body)


class PointerProvenance(MIRTransform):
    """Canonicalize indirect objects from current SSA pointer facts."""

    name = "provenance"

    def transform(self, body: MirBody) -> MirBody:
        from qbopt.analysis import alias

        return alias.annotated(body)


def pipeline(where: Where, **wanted) -> list[MIRTransform]:
    """The passes, in order, that `wanted` leaves on.

    Order is the list's own. A pass that is off is not in it, rather than in
    it and skipped, so what runs is what this returns.
    """
    every: list[MIRTransform] = [
        # Pointer identity is a solved program fact, not a frontend code-shape
        # requirement.  Resolve it before a packed pointer becomes independent
        # offset and selector values which no longer name the original address.
        PointerProvenance(),
        SplitPointers(),
        # Aggregate/object leaves become ordinary SSA before any scalar or
        # CFG pass asks what is constant, redundant, or loop invariant.
        promote.Sroa(where),
        Fold(where),
        Decide(where),
        loopsimplify.LoopSimplify(),
        lcssa.LoopClosedSSA(),
        # Strict floating recurrences must retain their original iteration
        # order.  Their numeric exit proof is canonical here; LICM may move
        # invariant x87 preparation out of the latch afterwards.
        FloatLoop(where),
        Hoist(where),
        DropStores(where),
        Gvn(where),
        # Ordinary scalar write-through promotion remains after memory GVN;
        # moving all of mem2reg ahead of loop normalization inflated matmul
        # from 165 to 226 instructions by creating loop phis too early.
        promote.Promote(where),
        strength.Strength(where),
        Algebraic(),
        Dead(),
        Place(where),
        unroll.Unroll(where),
        peel.Peel(where),
        fill.Fill(),
    ]
    return [one for one in every if wanted.get(one.name, True)]


# The order, from the pipeline itself rather than beside it: two lists that
# have to agree are one that will not.
PASSES = tuple(one.name for one in pipeline(Where()))
# What runs with no caller asking otherwise, which is not the same
# list: a pass may exist, be correct, and be off because it costs.
PASSES_ON = (
    tuple(one.name for one in pipeline(Where(), **_DEFAULTS))
    if False
    else tuple(one for one in PASSES if one != "strength")
)


@consts.reusing()
def applied(
    body: MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    *,
    blocks: list | None = None,
    found=None,
    fold: bool = True,
    lcssa_: bool = True,
    decide: bool = True,
    dead: bool = True,
    hoist: bool = True,
    forward: bool = True,
    drop_loads: bool = True,
    drop_stores: bool = True,
    promote_: bool = True,
    strength_: bool = True,
    floatloop_: bool = True,
    unroll_: bool = True,
    peel_: bool = True,
    fill_: bool = True,
    unswitch_: bool = False,
    only: str | None = None,
    registers: int | None = None,
    call_registers: int = 0,
    index_scales: frozenset[int] | None = None,
    address_forms: tuple[AddressForm, ...] | None = None,
    costs: OperationCosts | None = None,
    max_unroll_iterations: int = DEFAULT_MAX_UNROLL_ITERATIONS,
    max_unrolled_operations: int = DEFAULT_MAX_UNROLLED_OPERATIONS,
    coverage: dict[int, tuple[tuple[int, int], ...]] | None = None,
    watch=None,
) -> MirBody:
    """Every transform this module has, or the one `only` names.

    The absorb pass is gone. It emitted machine operations for the four
    long-arithmetic runtime calls, which is the machine arm's job and
    calls.py's already: the pass ran only under `--no-absorb-calls`, so
    with the default settings it found nothing and no gate exercised it.
    298 lines and 65 of this file's machine references went with it, and
    the plan's answer -- absorption at the raise -- is what replaces it.

    The place pass is gone with absorb and widen. It sank a definition
    towards its use -- the one transform here that changed the order
    instructions run in -- was off by default, bought nothing measured, and
    asked `origin` which other operations touched the same register, which
    is an interference question and the allocator's.
    """
    wanted = {
        # Every pass can be turned off, which is how a miscompile is
        # bisected: a variant that skips one and still allocates and emits
        # is the only kind that measures anything.
        "lcssa": lcssa_,
        "floatloop": floatloop_,
        "fold": fold,
        "decide": decide,
        "dead": dead,
        "hoist": hoist,
        "gvn": forward and drop_loads,
        "drop_stores": drop_stores,
        # Recurrences currently replace multiplication chains in innermost
        # loops. Shift-only and outer-loop formulas need pressure costing.
        "sroa": promote_,
        "promote": promote_,
        "strength": strength_,
        "unroll": unroll_,
        "peel": peel_,
        "fill": fill_,
    }
    where = Where(
        dgroup=dgroup,
        calls=calls,
        bounds=module.landmarks(found) if found is not None else None,
        blocks=blocks,
        found=found,
        # From the model, which is where MIR's register knowledge already
        # is. It belongs to the caller once the drivers thread it.
        registers=len(mir.TRACKED) if registers is None else registers,
        call_registers=call_registers,
        # Existing direct MIR callers retain native medium-model addressing.
        # Public frontends pass the complete, costed form set: 386 SIB is
        # legal through 67h, but is not silently treated as native or free.
        index_scales=frozenset({1}) if index_scales is None else index_scales,
        address_forms=() if address_forms is None else address_forms,
        costs=costs or OperationCosts(),
        max_unroll_iterations=max_unroll_iterations,
        max_unrolled_operations=max_unrolled_operations,
    )
    # These names were public debugging selectors before value reuse became
    # one pass.  Keep them as aliases rather than accepting a command that
    # now runs no pass at all; stage output itself uses the canonical name.
    if only in {"forward", "drop_loads", "reuse", "cse"}:
        only = "gvn"
    passes = [one for one in pipeline(where, **wanted) if only is None or one.name == only]
    # Pointer decomposition and SROA establish the scalar memory shape at
    # structural boundaries.  They are not members of the scalar fixed point:
    # rerunning global range analysis after every scalar round made the C
    # corpus take four times as long while producing identical code.  Exact
    # loop cloning can expose new packed accesses or fixed aggregate leaves,
    # though, so each structural candidate crosses this boundary once before
    # its ordinary scalar convergence.  Keep pipeline order here: packed far
    # references must expose their offset and selector before SROA reasons
    # about the memory object they address.
    structural = (PointerProvenance, SplitPointers, promote.Sroa)
    boundary = [one for one in passes if isinstance(one, structural)]
    passes = [one for one in passes if not isinstance(one, structural)]
    unrollers = [one for one in passes if isinstance(one, unroll.Unroll)]
    passes = [one for one in passes if not isinstance(one, unroll.Unroll)]
    peelers = [one for one in passes if isinstance(one, peel.Peel)]
    passes = [one for one in passes if not isinstance(one, peel.Peel)]

    def scalarized(state: MirBody, stage: str) -> MirBody:
        for one in boundary:
            state = one.transform(state)
            if watch is not None:
                watch(f"{stage}-{one.name}", state)
        return state

    body = scalarized(body, "r01")
    if only is not None and boundary:
        return _unreachable(body)
    if only is not None and unrollers:
        body = unrollers[0].transform(body)
        if watch is not None:
            watch("r01-unroll", body)
        return _unreachable(body)
    if only is not None and peelers:
        body = peelers[0].transform(body)
        if watch is not None:
            watch("r01-peel", body)
        return _unreachable(body)

    def fixed(state: MirBody, *, consider_unroll: bool = False, prefix: str = "") -> MirBody:
        # A monotone chain may expose one simplification per operation.
        # Scale with the body and separately reject a repeated state, so an
        # oscillator fails immediately instead of consuming that allowance.
        size = sum(1 + len(block.phis) + len(block.ops) for block in state.blocks)
        limit = max(16, size + 1)
        history = [state]
        for iteration in range(limit):
            before = state
            for one in passes:
                state = one.transform(state)
                if watch is not None:
                    watch(f"{prefix}r{iteration + 1:02d}-{one.name}", state)
            # Ask at the original pipeline boundary. Fully converging the
            # scalar passes first destroys matmul's exact counted-loop shape;
            # accepting the candidate still requires a separately converged
            # result, so profitability never compares a rough expansion.
            if consider_unroll and unrollers:
                state = unroll.optimized(
                    state,
                    where,
                    optimize=lambda candidate: structural_candidate(
                        candidate,
                        stage=f"{prefix}candidate-unroll",
                        prefix=f"{prefix}candidate-unroll-",
                    ),
                    watch=(None if watch is None else lambda stage, candidate: watch(f"{prefix}{stage}", candidate)),
                )
            if only is not None or state == before:
                # A structural candidate can make its last cloned region
                # unreachable on the same round that reaches the scalar
                # fixed point.  No later Decide pass then has a reason to
                # run `_unreachable`, so normalize the public boundary
                # itself: detached blocks retain ownership and no work.
                return _unreachable(state)
            if any(state == previous for previous in history):
                raise RuntimeError(f"MIR optimization did not converge: cycle after {iteration + 1} rounds")
            history.append(state)
        raise RuntimeError(f"MIR optimization did not converge after {limit} size-scaled rounds")

    def structural_candidate(
        candidate: MirBody,
        *,
        stage: str,
        prefix: str,
        consider_unroll: bool = False,
    ) -> MirBody:
        """Normalize addresses and newly exact leaves before pricing a CFG clone.

        Structural cloning crosses pointer decomposition and SROA before
        scalar convergence.  That convergence can itself make indexed
        accesses singleton leaves, so a second boundary is part of the same
        candidate transaction.  Only scalar MIR reconverges after that
        boundary: a further structural choice belongs to the next
        independently priced transaction.
        """
        state = fixed(
            scalarized(candidate, stage),
            consider_unroll=consider_unroll,
            prefix=prefix,
        )
        scalar = scalarized(state, f"{stage}-settled")
        if scalar == state:
            return state
        # The unscalarized side already pays for each explicit aggregate
        # load/store.  Charging its address and stored values as full-lived
        # register residents as well counted the same memory representation
        # twice (matmul: estimated 8,815 versus 3,396 after allocation).
        # Once SROA removes those homes, however, every retained SSA leaf has
        # to fit the finite register file or be recreated in a spill slot.
        before = profit.weighted(state, where.costs)
        settled = fixed(scalar, prefix=f"{prefix}settled-")
        after = profit.pressure_adjusted(settled, where.costs, where.registers)
        if before is not None and after is not None and after > before:
            if watch is not None:
                watch(f"{stage}-settled-rejected-pressure", settled)
            return state
        return settled

    body = fixed(body, consider_unroll=bool(unrollers))
    if peelers:
        body = peel.optimized(
            body,
            where,
            optimize=lambda candidate: structural_candidate(
                candidate,
                stage="candidate-peel",
                consider_unroll=bool(unrollers),
                prefix="candidate-peel-",
            ),
            watch=watch,
        )
    if unswitch_:
        from qbopt.optimize import unswitch

        body = unswitch.optimized(
            body,
            dgroup,
            calls,
            registers=where.registers,
            call_registers=where.call_registers,
            index_scales=where.index_scales,
            address_forms=where.address_forms,
            costs=where.costs,
            max_unroll_iterations=where.max_unroll_iterations,
            max_unrolled_operations=where.max_unrolled_operations,
            watch=watch,
        )
    return _unreachable(body)
