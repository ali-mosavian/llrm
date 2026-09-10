"""
MIR transforms: a body in, an optimised body out.

Everything else in this project changes a program by patching BC's bytes.
This changes the values, and layout.py turns what is left into bytes -- so a
transform here says what the program computes and nothing about how it was
written. That is the difference the roadmap calls retiring the machine arm.

Three of them so far, each the MIR statement of a pass that already exists
against the machine code, and each measured there first:

  widening      wide.pairs() finds the add/adc pair; lift.py emits it today
  redundant     avail.redundant() finds the reload; forward.py deletes it
  dead stores   avail.dead_stores() finds it; memory.py deletes it

Two of the three are on and find nothing extra, which is what parity means:
rewrite.py has already run forward.py and memory.py by the time a body
reaches here, so there is nothing left for them to take. Their worth is that
the machine arm can be deleted, not that they save a byte today. Widening is
off and applied() says why.

**Every byte of the original body has to stay accounted for.** layout.py
refuses a body it cannot cover, which is how it catches data BC put between
the instructions, and a transform that deletes an op leaves a hole in that
arithmetic. So nothing here deletes bytes: a survivor takes over the range,
through `Op.covers`, and the ops that were there are gone from the list.
That is bookkeeping about the *input*, not about what gets emitted -- the
output is whatever the surviving ops select to, which is shorter.

Each transform is separately switchable, deliberately. They interact -- a
widened pair changes which loads are redundant -- and a wrong answer from
one is otherwise a bisect through all three.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import avail
from qbopt.frontend import pairs
from qbopt.analysis import consts
from qbopt.objectfile import module
from qbopt.model.mir import Op
from qbopt.optimize import promote
from qbopt.optimize import strength
from qbopt.optimize import algebraic
from qbopt.optimize import loopmotion
from qbopt.optimize import unroll
from qbopt.optimize import lcssa
from qbopt.model.mir import MirBody
from qbopt.objectfile.module import Space
from qbopt.model.passes import Where
from qbopt.analysis import loops as loopy
from qbopt.model.passes import MIRTransform
from qbopt.analysis import liveness as alive_at
from qbopt.analysis.ssa import provider as _provider
from qbopt.analysis.ssa import pruned_phis as _pruned_phis
from qbopt.analysis.ssa import substituted as _substituted


def _end_of(op: Op) -> int:
    """One past this op's last original byte.

    `covers` is filled at the raise for every operation, so this no longer
    has to decode the instruction to find out where it ended.
    """
    return op.covers[1] if op.covers is not None else op.at


def _absorb(ops: list[Op], gone: set[int]) -> list[Op]:
    """`ops` without the ones whose address is in `gone`."""
    return _without(ops, lambda one: one.at in gone)


def _without(ops: list[Op], drop) -> list[Op]:
    """`ops` without the ones `drop` picks, their bytes given to a survivor.

    Backwards, so a run of deletions collapses onto the one op before them
    rather than each taking the next. The first op in a block has nothing
    before it, so a deletion there is refused by giving it to the op after
    -- and where there is neither, the body is one op long and there is
    nothing to delete.
    """
    out: list[Op] = []
    for op in ops:
        if drop(op):
            if (op.covers is not None and op.covers[0] == op.covers[1]
                and not op.extra_covers and op.floating_origin is None):
                continue
            # The bytes go to the op immediately before, and only if that op
            # is adjacent and gets its length from select.py. Anything
            # further back would span the survivors in between and count
            # their bytes twice; anything emitted verbatim is exactly as long
            # as the bytes it copies, so giving it more to account for makes
            # it disagree with itself -- qb-qrender's SCREEN.OBJ, whose
            # restore idiom stopped coming back its own length.
            #
            # Where neither holds the op simply stays. A deletion this cannot
            # account for is not one worth making.
            start = op.covers[0] if op.covers is not None else op.at
            if out and mir.rewritable(out[-1]) and _end_of(out[-1]) == start:
                lo = out[-1].covers[0] if out[-1].covers is not None else out[-1].at
                out[-1] = replace(
                    out[-1], covers=(lo, _end_of(op)), extra_covers=out[-1].extra_covers + op.extra_covers
                )
                continue
            out.append(op)
            continue
        out.append(op)
    if (
        len(out) > 1
        and drop(out[0])
        and mir.rewritable(out[1])
        and _end_of(out[0]) == (out[1].covers[0] if out[1].covers is not None else out[1].at)
    ):
        first, survivor = out[:2]
        start = first.covers[0] if first.covers is not None else first.at
        out[:2] = [
            replace(
                survivor,
                at=first.at,
                covers=(start, _end_of(survivor)),
                extra_covers=survivor.extra_covers + first.extra_covers,
            )
        ]
    return out


def widened(body: MirBody, dead: frozenset[int] = frozenset()) -> MirBody:
    """Every chain worth widening, as 32-bit operations on one register.

    This was wrong once and is worth saying how, because the fix was not the
    part that looked wrong. It renamed `add ax,[x]` with `adc dx,[x+2]` to
    `add eax,[x]`, and BC keeps that long in dx:ax -- so the carry landed in
    eax's high half and dx kept what it held. The rename itself is right;
    what was missing either side of it is:

    - the **chain**. One pair widened in isolation says nothing about where
      the long came from or goes. qbopt/frontend/pairs.py answers that -- six of its
      shapes agree with lift.py exactly, object by object.
    - the **restore**. `push eax / pop ax / pop dx` hands the long back to
      BC's sixteen-bit code, and without it every later read of dx is stale.
    - the **cost**. Two instructions become one plus a four-byte restore, so
      a lone pair widened is longer than what BC wrote. 235 of qb-qrender's
      341 chains would grow.

    All three live in pairs.py; this is where they are applied.
    """
    return pairs.widened(body, dead)


def without_redundant_loads(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """Every load whose destination already held what it loads, removed.

    The removal is the easy half. The load *defined* a value, and deleting
    it takes away that value's only definition -- so every later reader has
    to be told to read the provider instead, and told through every route a
    value reaches a reader by: an ordinary use, a `Held` operand, the base
    or segment of a memory operand, and a phi's incoming edge.

    Substituting by value, not by register. The two agree here -- the load
    is redundant precisely because the provider is already in the
    destination -- but saying it that way would make the deletion depend on
    an allocation that has not happened, and `origin` is what a body was
    raised with rather than what it will be emitted as.

    Without the substitution arridx miscompiled in all nine ordinary
    configurations: the add after the deleted load went on naming a value
    nobody wrote, allocation had no constraint to honour and put that
    phantom in bx, and `mov ax,bx` overwrote the product `imul` had just
    left in ax. VBDOS answered -26096, PDS 0 and QuickBASIC 4.5 11008,
    where the program prints 1260.
    """
    found = avail.redundant(body, dgroup, calls)
    if not found:
        return body
    gone = {at for at, _made, _who in found}
    swap = {made.id: who for _at, made, who in found}
    return replace(
        body,
        blocks=tuple(
            replace(
                one,
                phis=tuple(_phi_reading(phi, swap) for phi in one.phis),
                ops=tuple(_reading(op, swap) for op in _absorb(list(one.ops), gone)),
            )
            for one in body.blocks
        ),
    )


def _reading(op: Op, swap: dict) -> Op:
    """One operation reading the provider wherever it read the deleted load."""
    return _substituted(op, swap)


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
        mir.Kind.COPY,
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


def subexpressions(body: MirBody, dgroup: frozenset[int] = frozenset()) -> MirBody:
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
    from qbopt.analysis import floatbounds, floatfacts
    exact = floatfacts.known(body, dgroup, {}) if any(
        op.floating for block in body.blocks for op in block.ops) else {}
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
            key = _computation(op, stands, whole)
            if key is None:
                continue
            candidates = seen.setdefault(key, [])
            first = next((candidate for candidate in reversed(candidates)
                          if _reaches(candidate[0], candidate[1], order[block.at], index, doms, body, block)), None)
            if first is None:
                candidates.append((order[block.at], index, op))
                continue
            at, where, earlier = first
            if op.floating is not None and not _exact_float_path(
                body, body.blocks[at].at, where, block.at, index, exact, bounded
            ):
                candidates.append((order[block.at], index, op))
                continue
            if op.loads and (at != order[block.at] or not _undisturbed(op, earlier, block.ops[where + 1:index], dgroup)):
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
        body = replace(body, blocks=tuple(replace(block, ops=tuple(
            _erased_floating(op) if id(op) in floating_gone else op for op in block.ops
        )) for block in body.blocks))
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


def _exact_float_path(
    body: MirBody, source: int, first: int, destination: int, last: int, facts: dict, bounded: set[int],
) -> bool:
    """All operations between dominating FP candidates are exception-free.

    Acyclic paths may cross diamonds; cycles need a separate environment
    invariant proof. Loads still require the independent memory guard.
    """
    blocks = {block.at: block for block in body.blocks}
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
        exact = all(id(op) in bounded or _exact_floating(op, facts) for op in ops[start:end])
        parents = predecessors[at]
        result = exact and (at == source or (bool(parents) and all(visit(parent) for parent in parents)))
        active.remove(at)
        checked[at] = result
        return result

    return visit(destination)


def _exact_stored_load(op: Op, facts: dict) -> Op | None:
    """An exact storage conversion leaves the source value available for reloads."""
    from qbopt.model.floating import Format, Precision, Rounding, Semantics
    if (op.kind is not mir.Kind.FSTORE or op.floating is None
        or op.floating.inputs != (Format.EXTENDED80,)
        or op.floating.result not in (Format.BINARY32, Format.BINARY64)
        or len(op.stores) != 1 or op.loads or not _exact_floating(op, facts)):
        return None
    source, = op.args
    if not isinstance(source, mir.Held) or source.width != 10:
        return None
    rule = Semantics((op.floating.result,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE,
                     op.floating.exceptions)
    return replace(op, kind=mir.Kind.FLOAD, args=(mir.Cell(op.stores[0]),), results=(source,),
                   defines=(source.value,), uses=(), loads=op.stores, stores=(), merges={}, floating=rule)


def _exact_floating(op: Op, facts: dict) -> bool:
    """No intervening exceptional FP work or unmodelled environment change."""
    from qbopt.analysis import floatfacts
    if op.barrier or op.kind in (mir.Kind.CALL, mir.Kind.OPAQUE):
        return False
    if op.floating is None:
        return op.stack is None
    if op.kind is mir.Kind.FSTORE:
        return (len(op.args) == 1 and isinstance(op.args[0], mir.Held)
                and op.args[0].value in facts
                and floatfacts.evaluated(op.kind, op.floating, (facts[op.args[0].value],)) is not None)
    return (len(op.results) == 1 and isinstance(op.results[0], mir.Held)
            and op.results[0].value in facts)


def _erased_floating(op: Op) -> Op:
    return replace(op, op=ir.Operation.NOTHING, kind=mir.Kind.NOTHING, name="",
                   args=(), results=(), uses=(), defines=(), loads=(), stores=(),
                   merges={}, node=None, made=None, raised=None, floating=None, stack=None)


def _reclaimed(body: MirBody, gone: set[int]) -> MirBody:
    """`body` without those operations, their bytes given to a neighbour.

    Whole-body rather than within a block, which is what `_without` does and
    why it could not be used here. The raise gives every operation folded
    out of a runtime call the same `at` -- the site's first push -- so `at`
    says nothing about which bytes an operation stands for, and the
    operation before it in its own block is routinely somewhere else
    entirely. `covers` is the fact; the op that ends where this one starts
    is its neighbour wherever it lives.

    An operation whose bytes nothing can take simply stays. Its uses are
    still substituted, so it computes something nothing reads and the next
    round's `dead` sees it -- a deletion this cannot account for is not one
    worth making.
    """
    ends: dict[int, Op] = {}
    for block in body.blocks:
        for op in block.ops:
            if id(op) not in gone and mir.rewritable(op) and op.covers is not None:
                ends[op.covers[1]] = op
    grown: dict[int, tuple[int, int]] = {}
    extra: dict[int, tuple] = {}
    dropped: set[int] = set()
    # In byte order, so a run of deletions collapses onto the one operation
    # standing before all of them. Taken in block order, the second of two
    # adjacent deletions looks for a neighbour that is itself going.
    going = sorted(
        (op for block in body.blocks for op in block.ops if id(op) in gone and op.covers is not None),
        key=lambda one: one.covers,
    )
    for op in going:
        taker = ends.get(op.covers[0])
        if taker is None:
            continue
        span = grown.get(id(taker), taker.covers)
        grown[id(taker)] = (span[0], op.covers[1])
        extra[id(taker)] = extra.get(id(taker), taker.extra_covers) + op.extra_covers
        ends.pop(op.covers[0], None)
        ends[op.covers[1]] = taker
        dropped.add(id(op))
    if not dropped:
        return body
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    replace(op, covers=grown[id(op)], extra_covers=extra[id(op)]) if id(op) in grown else op
                    for op in block.ops
                    if id(op) not in dropped
                ),
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
        mir.Kind.FLOAD, mir.Kind.FADD, mir.Kind.FSUB, mir.Kind.FMUL, mir.Kind.FDIV)
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
        elif isinstance(one, mir.Cell):
            ref = mir._symbolic_ref(one.ref)
            if not mir.same_bytes(ref, ref):
                return None
            named.append(("m", ref))
        else:
            return None
    results = tuple(one.width for one in op.results if isinstance(one, mir.Held))
    operands = frozenset(named) if len(named) == 2 and op.kind in {
        mir.Kind.ADD, mir.Kind.MUL, mir.Kind.AND, mir.Kind.OR, mir.Kind.XOR,
        mir.Kind.EQ, mir.Kind.NE,
    } else tuple(named)
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
            node=None,
            made=None,
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
    for ref in one.loads + one.stores:
        for other in run:
            for theirs in other.loads + other.stores:
                if mir.overlapping(ref, theirs, dgroup):
                    return False
    return True


def without_dead_stores(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """Every store overwritten before anything read it, removed."""
    gone = {id(op) for op in avail.dead_stores(body, dgroup, calls)}
    if not gone:
        return body
    return replace(
        body,
        blocks=tuple(replace(one, ops=tuple(_without(list(one.ops), lambda op: id(op) in gone))) for one in body.blocks),
    )


# A root register at the width an operand reads it. ir.ROOT maps the narrow
# name to the wide one; this is the way back, and only for the general
# registers -- a segment register has no narrower form and is never a
# provider here.


def forwarded(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """Replace known memory operands with SSA values, extending their uses.

    Arithmetic remains intact. The allocator, not this pass, decides where
    the longer-lived provider resides.
    """
    body = _floating_forwarded(body, dgroup, calls)
    want = frozenset(op.at for block in body.blocks for op in block.ops if op.loads)
    if not want:
        return body
    served = {id(one.op): one.value for one in avail.forwardable(body, dgroup, calls, want) if one.value is not None}
    if not served:
        return body

    out = []
    for block in body.blocks:
        ops: list[Op] = []
        for op in block.ops:
            holder = served.get(id(op))
            args = _served(op, holder) if holder is not None else None
            ops.append(op if args is None else replace(op, args=args, loads=(), uses=op.uses + (holder,)))
        out.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(out))


def _floating_forwarded(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """Exact stored values can replace arithmetic memory inputs without rounding anew."""
    from qbopt.analysis import floatbounds, floatfacts
    from qbopt.model.floating import Format
    if not any(op.kind is mir.Kind.FSTORE for block in body.blocks for op in block.ops):
        return body
    facts = floatfacts.known(body, dgroup, calls)
    bounded = floatbounds.exact(body, facts, dgroup)
    blocks = []
    for block in body.blocks:
        available = []
        ops = []
        for op in block.ops:
            changed = op
            if (not op.barrier and op.kind in (mir.Kind.FADD, mir.Kind.FSUB, mir.Kind.FMUL, mir.Kind.FDIV)
                and op.floating is not None and len(op.args) == len(op.floating.inputs)):
                args, formats = list(op.args), list(op.floating.inputs)
                for index, (arg, format) in enumerate(zip(args, formats)):
                    if not isinstance(arg, mir.Cell):
                        continue
                    provider = next((one for one in reversed(available)
                                     if one.floating.inputs == (format,)
                                     and mir.same_bytes(one.loads[0], arg.ref)), None)
                    if provider is not None:
                        args[index] = provider.results[0]
                        formats[index] = Format.EXTENDED80
                if tuple(args) != op.args:
                    changed = replace(op, args=tuple(args),
                        loads=tuple(arg.ref for arg in args if isinstance(arg, mir.Cell)),
                        uses=tuple(dict.fromkeys((*op.uses, *(arg.value for arg in args if isinstance(arg, mir.Held))))),
                        floating=replace(op.floating, inputs=tuple(formats)))
            ops.append(changed)
            available = [one for one in available if not any(
                mir.overlapping(one.loads[0], written, dgroup) for written in op.stores)]
            if id(op) not in bounded and not _exact_floating(op, facts):
                available.clear()
            provider = _exact_stored_load(op, facts)
            if (id(op) in bounded and op.kind is mir.Kind.FLOAD
                and len(op.loads) == len(op.results) == 1 and not op.stores
                and isinstance(op.results[0], mir.Held) and op.results[0].width == 10):
                provider = op
            if provider is not None:
                available.append(provider)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


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
    return tuple(mir.Held(holder, cell.ref.width) if one is cell else one for one in op.args)


SEGMENT_NAMES = frozenset({"es", "fs", "gs"})


def _segment_load(op: Op):
    """(which resource, what it is loaded from) where this op loads one.

    A descriptor lands in a machine resource MIR has no value for, so it
    arrives as mir.Opaque carrying that resource's own name. This read the
    register number out of the instruction.
    """
    if op.kind is not mir.Kind.LOAD or len(op.results) != 1 or len(op.args) != 1:
        return None
    into = op.results[0]
    if not isinstance(into, mir.Opaque) or into.name not in SEGMENT_NAMES:
        return None
    return into.name, op.args[0]


def segments(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """A segment register loaded from what it already holds, dropped.

    `mov es,[desc+2]` twice in one statement, because the element appears
    twice in it -- and again on every pass of the loop, from a word written
    once before the loop ran. `segments.py` does this against the machine
    code and finds 42 sites in qb-qrender; nothing in MIR did.

    Cross-block, and it has to be: what a block starts holding is what every
    path into it agrees on, and the reload that matters most arrives over a
    back-edge. Starting from "nothing is held" and growing is the
    conservative direction -- a cycle cannot talk itself into a fact.

    Hoisting the survivor out of the loop is a separate and larger thing,
    and needs code motion between blocks. This only removes the duplicate.
    """
    blocks = {block.at: block for block in body.blocks}
    preds: dict[int, list[int]] = {at: [] for at in blocks}
    for block in body.blocks:
        for successor in block.succ:
            if successor in preds:
                preds[successor].append(block.at)

    def through(block, holding: dict, gone: set[int] | None):
        holding = dict(holding)
        for op in block.ops:
            if op.barrier or op.at in calls:
                holding = {}
                continue
            found = _segment_load(op)
            if found is not None:
                register, source = found
                if holding.get(register) == source:
                    if gone is not None:
                        gone.add(op.at)
                elif source is not None:
                    holding[register] = source
                continue
            # A write through the segment itself cannot be the descriptor
            # it was loaded from. A dynamic array's storage is outside
            # DGROUP -- that is the whole reason it needs a segment -- and
            # the descriptor is a LITERAL or SEGMENT cell reached through
            # ds. Anything else that writes memory, or writes somewhere
            # this cannot name, puts the descriptor back in doubt.
            if any(one.addr is None or one.addr.space is not Space.FAR for one in op.stores):
                holding = {}
        return holding

    exits = {at: {} for at in blocks}
    for _round in range(len(blocks) + 1):
        changing = False
        for at in sorted(blocks):
            entering: dict | None = None
            for previous in preds[at]:
                was = exits[previous]
                entering = dict(was) if entering is None else {k: v for k, v in entering.items() if was.get(k) == v}
            got = through(blocks[at], entering or {}, None)
            if got != exits[at]:
                exits[at] = got
                changing = True
        if not changing:
            break

    gone: set[int] = set()
    for at in sorted(blocks):
        entering = None
        for previous in preds[at]:
            was = exits[previous]
            entering = dict(was) if entering is None else {k: v for k, v in entering.items() if was.get(k) == v}
        through(blocks[at], entering or {}, gone)
    if not gone:
        return body
    return replace(
        body,
        blocks=tuple(replace(one, ops=tuple(_absorb(list(one.ops), gone))) for one in body.blocks),
    )


# Operations with a register operand the encoding does not name: the
# one-operand `imul`/`idiv` whose other half is dx:ax, `cwd` and `cdq`, and
# a shift by cl. select.py cannot remap what is not an operand.


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
            for use in op.uses:
                # Really read, not merely preserved. `merges` is the raise's
                # answer to that; asked as "is this use's register among the
                # ones the instruction names", the index register of a based
                # operand was missed and the use read as preserved.
                if use.flags or op.at in calls or use not in op.merges:
                    wanted.add(use)
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
        0, (1 << (divisor.width * 8)) - 1,
    )


def _whole_shift(op: Op, readable: set | None) -> bool:
    """A complete scalar definition needs no loop-carried destination contents."""
    match op.kind, op.args, op.results:
        case mir.Kind.SHL, (mir.Held(width=width), mir.Const(n=count)), (mir.Held(width=result_width),):
            return (width == result_width and 0 < count < width * 8 and not op.merges
                    and readable is not None
                    and not any(value.flags and value in readable for value in op.defines))
        case _:
            return False


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
    if any(one.kind in (mir.Kind.CALL, mir.Kind.ESCAPE) or one.barrier for one in ops):
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
            if one in run or one.stores or one.floating is not None or not real:
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
            if one.kind is mir.Kind.DIVMOD and not _cannot_fault(one):
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
                one.kind is mir.Kind.COPY and not one.loads and not any(use in made for use in one.uses)
                and not (len(one.args) == 1 and isinstance(one.args[0], mir.Symbol))
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
            if any(
                value in begins and value in twice and (readable is None or value in readable)
                for value in one.defines
                if not value.flags
            ) and not (
                one.kind is mir.Kind.COPY and not one.merges
                and len(one.args) == 1 and isinstance(one.args[0], mir.Symbol)
            ) and not _whole_shift(one, readable):
                continue
            # An operand nothing writes down used to end the run here.
            # hotlop hoisted `mov ax,[n] / imul word [k]`, the recolour
            # renamed the load to cx, and the multiply went on reading ax:
            # 0 for 630. Two things had to exist before that could be
            # allowed. regalloc.required() now refuses an assignment that
            # puts such an operand anywhere but where its instruction reads
            # it, and a result the machine places is copied out rather than
            # re-seated -- see `copied` below.
            if any(
                ref.addr is None or mir.overlapping(ref, other, dgroup, bounds, known=(intervals or {}).get(id(one)))
                for ref in one.loads for other in stores
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
            blocking = [use for use in one.uses if not use.flags and use not in one.merges]
            if any(use in inside and use not in made for use in blocking):
                continue
            run.append(one)
            made.update(one.defines)
            changing = True

    thinning = True
    while thinning:
        thinning = False
        for one in run:
            if one.kind is not mir.Kind.COPY or one.loads or any(isinstance(arg, mir.Symbol) for arg in one.args):
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
    # A phi carries a value out of the run as surely as an instruction
    # reads one, and `rest` holds no phis: hotlop's high half reached the
    # next iteration that way, seen by nothing here.
    taken = {value for other in rest for value in other.uses} | {
        value for phi in (phis or ()) for value in phi.incoming.values()
    }
    if wanted is not None:
        taken &= wanted
    crossing = {value for one in run for value in one.defines if value in taken}
    if not crossing or any(value.flags for value in crossing):
        return None
    return frozenset(crossing)


def _span_of(op: Op) -> tuple[int, int] | None:
    """The bytes this operation occupied before anything moved it."""
    return op.covers


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
        mir.Kind.FCOMPARE,
        mir.Kind.FCHECK,
    }
)


def _leaving(body: MirBody) -> set:
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


def _outcome(block, op: Op, facts: dict, held: dict) -> bool | None:
    """Whether this branch is taken, where both its operands are numbers."""
    if op.kind is not mir.Kind.BRANCH:
        return None
    decide = _TAKEN.get(op.test)
    if decide is None:
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
    if compare.kind is not mir.Kind.SUB or len(compare.args) != 2 or compare.results:
        return None
    parts = [consts._operand(compare, one, facts, held.get((block.at, index))) for one in compare.args]
    if any(one is None for one in parts):
        return None
    left, right = parts
    return decide(_signed(left), _signed(right), lambda n: n & 0xFFFFFFFF)


def _threaded(body: MirBody) -> MirBody:
    """Bypass empty jump blocks without changing any incoming phi value."""
    known = {block.at: block for block in body.blocks}
    redirects = {}
    for block in body.blocks:
        if block.phis or len(block.succ) != 1 or not block.ops:
            continue
        last = block.ops[-1]
        if last.kind is not mir.Kind.JUMP or last.target != block.succ[0]:
            continue
        if any(op.kind is not mir.Kind.NOTHING or op.defines or op.uses or op.loads or op.stores
               or op.barrier or op.floating is not None or op.stack is not None for op in block.ops[:-1]):
            continue
        successor = known.get(last.target)
        if successor is not None and not successor.phis:
            redirects[block.at] = successor.at
    blocks = []
    changed = False
    for block in body.blocks:
        if not block.ops or block.ops[-1].kind not in {mir.Kind.JUMP, mir.Kind.BRANCH}:
            blocks.append(block)
            continue
        last = block.ops[-1]
        target = last.target
        seen = {block.at}
        while target in redirects and target not in seen:
            seen.add(target)
            target = redirects[target]
        if target == last.target or target in seen:
            blocks.append(block)
            continue
        changed = True
        successors = tuple(dict.fromkeys(target if at == last.target else at for at in block.succ))
        blocks.append(replace(block, ops=(*block.ops[:-1], replace(last, target=target)), succ=successors))
    return _unreachable(replace(body, blocks=tuple(blocks))) if changed else body


def decided(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """A branch on two numbers, resolved.

    `IF a < b` with both constants is decided where it stands, and leaving
    it to run costs the comparison, the jump, and everything on the arm
    that cannot be taken. bools is three of them over four constants and
    nothing else.

    Taken becomes an unconditional jump and not-taken goes entirely, its
    bytes handed to the operation before it. What becomes unreachable is
    dropped by resolving the body afterwards rather than here.
    """
    body = _threaded(body)
    facts = consts.known(body, dgroup, calls)
    if not facts:
        return body
    held = consts.cells(body, dgroup, calls, facts)

    out = []
    changed = False
    for block in body.blocks:
        if not block.ops:
            out.append(block)
            continue
        last = block.ops[-1]
        answer = _outcome(block, last, facts, held)
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
                made=None,
                covers=last.covers or _span_of(last),
            )
            out.append(replace(block, ops=block.ops[:-1] + (jump,), succ=(target,)))
        else:
            kept = _absorb(list(block.ops), {last.at})
            if len(kept) == len(block.ops):
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
            block
            if block.at in reached
            else replace(
                block,
                succ=(),
                phis=(),
                ops=tuple(
                    replace(
                        op,
                        kind=mir.Kind.NOTHING,
                        name="",
                        defines=(),
                        uses=(),
                        loads=(),
                        stores=(),
                        args=(),
                        results=(),
                        merges={},
                        made=None,
                        target=None,
                        test=None,
                        stack=None,
                        symbol=False,
                    )
                    for op in block.ops
                ),
            )
            for block in body.blocks
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
    limited = any(one.barrier or one.kind is mir.Kind.OPAQUE for block in body.blocks for one in block.ops)
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
            alive.update(value for block in body.blocks for op in block.ops
                         if any(value in alive for value in op.defines) for value in op.uses)
            if len(alive) == before:
                break
    out = []
    changed = False
    for block in body.blocks:
        # Several semantic operations may share an input address. Their
        # computations are independent even when their provenance is not.
        overwritten = _overwritten_locally(block) if limited else None
        gone = {id(op) for op in block.ops if _removable(op, alive)
                and (overwritten is None or set(op.defines) <= overwritten)}
        if not gone:
            out.append(block)
            continue
        ops = [
            replace(
                op,
                op=ir.Operation.NOTHING,
                name="",
                kind=mir.Kind.NOTHING,
                defines=(),
                uses=(),
                args=(),
                results=(),
                loads=(),
                merges={},
                node=None,
                made=None,
                raised=None,
                symbol=False,
            )
            if id(op) in gone
            else op
            for op in block.ops
        ]
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
        overwritten.update(value for value in op.defines
                           if value.version and (value.variable, value.flags) in written)
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
            made=None,
            raised=None,
            symbol=False,
            node=None,
            covers=op.covers if index == 0 else (op.at, op.at),
            id=op.id if index == 0 else None,
            extra_covers=op.extra_covers if index == 0 else (),
        )
        for index, (result, number) in enumerate(zip(op.results, numbers, strict=True))
    )


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
    floating_facts = floatfacts.known(body, dgroup, calls) if any(op.floating for block in body.blocks for op in block.ops) else {}
    conversions = floatfacts.converted(body, dgroup, calls, facts=floating_facts)
    argument_facts = facts | conversions
    memory = (
        consts.cells(body, dgroup, calls, facts, edges=edges)
        if any(op.loads or op.kind is mir.Kind.DIVMOD for block in body.blocks for op in block.ops)
        else {}
    )
    if not facts and not memory and not argument_facts:
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
            made = _constant_operands(_folded_op(op, facts, wanted),
                                      argument_facts if op.kind is mir.Kind.ARG else facts,
                                      memory.get((block.at, index), {}))
            changed = changed or made is not op
            ops.append(made)
        out.append(replace(block, ops=tuple(ops)))
    from qbopt.optimize import floatfold
    return floatfold.stored(floatfold.discarded(replace(body, blocks=tuple(out)) if changed else body, conversions), floating_facts)


def _constant_operands(op: Op, facts: dict, memory: dict | None = None) -> Op:
    """Propagate width-proven constants without reversing ordered operands."""
    if op.kind is mir.Kind.ARG:
        return _constant_argument(op, facts, memory or {})
    if (op.kind is mir.Kind.STORE and len(op.args) == len(op.stores) == 1
        and not op.defines and not op.merges and not op.loads and not op.barrier
        and op.floating is None and isinstance(arg := op.args[0], mir.Held)
        and arg.width == op.stores[0].width
        and (fact := facts.get(arg.value)) is not None and fact.width >= arg.width):
        address_values = {value for ref in op.stores for value in (ref.base, ref.segment) if value is not None}
        return replace(op, args=(mir.Const(consts.masked(fact.n, arg.width), arg.width),),
            uses=tuple(value for value in op.uses if value != arg.value or value in address_values),
            node=None, made=None, raised=None, symbol=False)
    if (
        op.kind not in (
            mir.Kind.ADD, mir.Kind.ADD_CARRY, mir.Kind.AND, mir.Kind.OR, mir.Kind.XOR,
            mir.Kind.MUL, mir.Kind.SUB, mir.Kind.SUB_BORROW, mir.Kind.DIVMOD,
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
            (not ordered or index == 1) and isinstance(arg, mir.Held)
            and (fact := facts.get(arg.value)) is not None and fact.width >= arg.width
        ):
            args.append(mir.Const(consts.masked(fact.n, arg.width), arg.width))
            replaced.add(arg.value)
        elif ((not ordered or index == 1) and isinstance(arg, mir.Cell)
              and not op.stores and not op.barrier and arg.ref in op.loads
              and (fact := consts._cell(memory or {}, arg.ref)) is not None):
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
        op, args=tuple(args),
        loads=tuple(ref for ref in op.loads if ref not in removed),
        node=None if removed else op.node,
        made=None if removed else op.made,
        raised=None if removed else op.raised,
        uses=tuple(value for value in op.uses if value not in replaced or value in op.merges or value in retained),
    )


def _constant_argument(op: Op, facts: dict, memory: dict) -> Op:
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
    fact = consts._operand(op, arg, facts, memory)
    if fact is None or fact.width < width:
        return op
    kept = tuple(ref for ref in op.loads if not isinstance(arg, mir.Cell) or ref != arg.ref)
    uses = tuple(dict.fromkeys(value for ref in (*kept, *op.stores)
                               for value in (ref.base, ref.segment) if value is not None))
    return replace(op, args=(mir.Const(consts.masked(fact.n, width), width),), uses=uses,
                   loads=kept, node=None, made=None, raised=None, symbol=False)


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
        made=None,
        node=None,
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
    wants = {
        value for one in run for value in one.uses if not value.flags and value not in one.merges and value not in made
    }
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
        # The renamed values keep their origin. "Where BC had it" is still
        # true of them and is what layout remaps an operand through -- drop
        # it and the operand keeps the register the instruction was raised
        # with, whatever the allocator decided. What the rename changes is
        # which variable a value is a version of, and nothing else.
        origin={value(one): where for one, where in body.origin.items()},
        pins={value(one): where for one, where in body.pins.items()},
    )


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
    intervals = {id(op): scoped.get(block.at, {}) for block in body.blocks for op in block.ops}
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
            if one.kind is mir.Kind.COPY and len(one.args) == 1 and isinstance(one.args[0], mir.Symbol)
            and all((value, HIGH) not in demanded for value in one.defines)
            else one
            for one in ops
        ]
        identities = {id(one): id(original) for one, original in zip(ops, originals, strict=True)}
        stores = [ref for one in ops for ref in one.stores]
        carried = {phi.result for at in loop.body for phi in at_of[at].phis}
        phis = [phi for at in loop.body for phi in at_of[at].phis]
        run = _invariant_run(ops, carried, stores, dgroup, calls, phis, bounds, _starts(phis), readable, intervals)
        # Track operations, not source addresses: hoisted definitions share
        # their anchor's address with other computations and the jump. HARR
        # lost all of those when its descriptor moved a second time.
        run = [one for one in run if identities[id(one)] not in gone]
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
            ops = [replace(first, at=block.ops[0].at, covers=first.covers or _span_of(first))] + ops[1:]
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
                lifted = [replace(one, at=anchor, covers=one.covers or _span_of(one)) for one in lifted]
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
        return decided(body, self.where.dgroup, self.where.named)


class Dead(MIRTransform):
    name = "dead"

    def transform(self, body: MirBody) -> MirBody:
        return dead(body)


class Segments(MIRTransform):
    name = "segments"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return segments(body, self.where.dgroup, self.where.named)


class Hoist(MIRTransform):
    name = "hoist"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        body = hoisted(body, self.where.dgroup, self.where.named, self.where.bounds)
        return loopmotion.sunk_stores(body, self.where.dgroup, self.where.bounds)


class Forward(MIRTransform):
    name = "forward"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return forwarded(body, self.where.dgroup, self.where.named)


class DropLoads(MIRTransform):
    name = "drop_loads"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return without_redundant_loads(body, self.where.dgroup, self.where.named)


class DropStores(MIRTransform):
    name = "drop_stores"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return without_dead_stores(body, self.where.dgroup, self.where.named)


class Reuse(MIRTransform):
    """Not in `pipeline` below, and must not be until the copy it leaves
    behind is emitted as one. Wired in, lngmix's second divide became the
    original `call` bytes carried verbatim -- the site's own -- and the
    program stopped early under DOSBox (e2e NODONE). The fold itself is
    right: one idiv leaves the loop and the image is 899 bytes against
    915. What is missing is the emission of an operation a pass invented
    standing at an absorbed site's address.
    """

    name = "reuse"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return reused_divides(body, self.where.dgroup, self.where.found)


class Cse(MIRTransform):
    name = "cse"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        from qbopt.optimize import floatfold, gvn
        canonical = subexpressions(body, self.where.dgroup)
        # Finish exposing existing providers before making a supposedly missing one.
        return floatfold.checks(gvn.joined(canonical, insert=canonical == body))


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


def pipeline(where: Where, **wanted) -> list[MIRTransform]:
    """The passes, in order, that `wanted` leaves on.

    Order is the list's own. A pass that is off is not in it, rather than in
    it and skipped, so what runs is what this returns.
    """
    every: list[MIRTransform] = [
        Fold(where),
        Decide(where),
        Segments(where),
        lcssa.LoopClosedSSA(),
        Hoist(where),
        Forward(where),
        DropLoads(where),
        DropStores(where),
        Reuse(where),
        Cse(where),
        promote.Promote(where),
        strength.Strength(where),
        Algebraic(),
        Dead(),
        Place(where),
        unroll.Unroll(where),
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
    segments_: bool = True,
    hoist: bool = True,
    forward: bool = True,
    drop_loads: bool = True,
    drop_stores: bool = True,
    promote_: bool = True,
    strength_: bool = True,
    unroll_: bool = True,
    only: str | None = None,
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
        "fold": fold,
        "decide": decide,
        "dead": dead,
        "segments": segments_,
        "hoist": hoist,
        "forward": forward,
        "drop_loads": drop_loads,
        "drop_stores": drop_stores,
        # Recurrences currently replace multiplication chains in innermost
        # loops. Shift-only and outer-loop formulas need pressure costing.
        "promote": promote_,
        "strength": strength_,
        "unroll": unroll_,
    }
    where = Where(
        dgroup=dgroup,
        calls=calls,
        bounds=module.landmarks(found) if found is not None else None,
        blocks=blocks,
        found=found,
    )
    passes = [one for one in pipeline(where, **wanted) if only is None or one.name == only]
    if found is not None:
        body = replace(
            body,
            blocks=tuple(
                replace(
                    block,
                    ops=tuple(
                        replace(op, extra_covers=found.coverage[op.id][1:])
                        if not op.extra_covers and op.id in found.coverage
                        else op
                        for op in block.ops
                    ),
                )
                for block in body.blocks
            ),
        )
    for iteration in range(16):
        before = body
        for one in passes:
            body = one.transform(body)
            if watch is not None:
                watch(f"r{iteration + 1:02d}-{one.name}", body)
        if only is not None or body == before:
            return body
    raise RuntimeError("MIR optimization did not converge after 16 rounds")
