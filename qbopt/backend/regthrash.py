"""Renaming a definition's register so a copy after it is redundant.

Open Watcom's `RegThrash` (`bld/cg/c/scthrash.c`), and it is the inverse of
coalescing. A coalescer asks, before registers are chosen, whether two
values may share one, and a conservative one refuses whenever it cannot
prove the merged range still colourable. This asks nothing: registers are
already chosen, so it takes

    OP(...) -> Y        ...        mov Z, Y     (and Y dies there)

and writes the producer's result into Z instead, which leaves the move
copying Z to itself. No interference graph, no colourability test, nothing
to refuse.

Measured on qbdemo: `coalesce` examines 509 copy pairs and refuses 220 of
them purely on Briggs, while only 57 pairs genuinely interfere. Every one
of those 220 is invisible to this pass, because the question it turns on
was never asked.

**What it must still check.** Between the definition and the move, Y has to
stay live and untouched and Z has to be dead -- otherwise the rename
clobbers something. And the rewritten instruction has to still exist: a
386 encodes `mov [si],ax` and has no form naming `[ax]`, so Watcom re-runs
instruction selection after the substitution and keeps the rename only if
it survives. `select.emit` is that check here.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model.passes import LIRTransform

# Enough to settle. Each pass makes at most one rename per block, because a
# rename changes the liveness every later candidate is judged against, and
# recomputing it per candidate costs more than going round again.
ROUNDS = 8


class RegThrash(LIRTransform):
    name = "regthrash"

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return thrashed(body)


def thrashed(body: lir.LirBody) -> lir.LirBody:
    for _round in range(ROUNDS):
        after = _once(body)
        if after is body:
            return body
        body = after
    return body


def _once(body: lir.LirBody) -> lir.LirBody:
    from qbopt.backend import liveness

    exits = liveness.dead_at_exit(body)
    blocks = list(body.blocks)
    for index, block in enumerate(blocks):
        done = _block(block, set(exits[block.at]))
        if done is not None:
            blocks[index] = done
            return replace(body, blocks=tuple(blocks))
    return body


def _dead_after(block: lir.LirBlock, dead: set) -> "dict[int, set] | None":
    """Per instruction, the register lanes dead once it has run."""
    from qbopt.backend import liveness
    from qbopt.backend.peephole import _flag_lanes
    from qbopt.backend.peephole import _register_effects

    out: dict[int, set] = {}
    for one in reversed(block.insns):
        out[id(one)] = set(dead)
        if liveness._terminator(one.what):
            if one.what.op is ir.Operation.BRANCH:
                dead -= _flag_lanes(0xFFFFFFFF)
            continue
        effects = _register_effects(one, flags=True)
        if effects is None:
            dead.clear()
            continue
        reads, writes = effects
        dead = (dead | writes) - reads
    return out


def _block(block: lir.LirBlock, dead: set) -> "lir.LirBlock | None":
    """This block with one copy thrashed away, or None where none can be."""
    from qbopt.backend.peephole import _lanes

    after = _dead_after(block, dead)
    for position, one in enumerate(block.insns):
        pair = _plain_copy(one)
        if pair is None:
            continue
        into, out_of = pair
        # Y has to die at the move. That is the whole licence for the
        # rename: if anything later reads Y, its definition still has to
        # land in Y and there is nothing to rewrite.
        if not _lanes(out_of.register) <= after[id(one)]:
            continue
        found = _producer(block, position, out_of, into, after)
        if found is None:
            continue
        at, tied = found
        rewritten = _renamed(block.insns[at], out_of.register, into.register, result_only=not tied)
        if rewritten is None:
            continue
        if not tied:
            # The producer never read Y, so writing Z instead is the whole
            # of it and the move has nothing left to do.
            insns = [rewritten if one == at else insn for one, insn in enumerate(block.insns) if one != position]
            return replace(block, insns=tuple(insns))
        # Two-address: the producer reads Y as well as writing it, so the
        # renamed form reads Z and Z has to arrive first. Watcom's
        # `PrefixIns` -- the copy is relocated, not removed, and what it
        # buys is Y's range ending here instead of at the old move.
        insns = []
        for one, insn in enumerate(block.insns):
            if one == position:
                continue
            if one == at:
                insns.append(block.insns[position])
                insns.append(rewritten)
            else:
                insns.append(insn)
        return replace(block, insns=tuple(insns))
    return None


def _plain_copy(one: lir.Insn) -> "tuple[ir.Reg, ir.Reg] | None":
    """`(written, read)` where this is a register-to-register move of one width."""
    what = one.what
    if what is None or one.clobbers or one.symbol is True or one.group is not None:
        return None
    if one.requires or one.delivers or one.spread:
        return None
    match what:
        case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg() as into,), (ir.Reg() as out_of,)):
            if into.width != out_of.width or into.register == out_of.register:
                return None
            return into, out_of
        case _:
            return None


def _producer(block, position: int, out_of: ir.Reg, into: ir.Reg, after) -> "tuple[int, bool] | None":
    """Where `out_of` was defined, and whether that definition reads it too.

    Watcom's backward walk. Every step has to leave Y live and untouched --
    otherwise the definition found is not the one the move reads -- and has
    to leave Z dead, or renaming into it destroys a value something else
    still wants.
    """
    from qbopt.backend import liveness
    from qbopt.backend.peephole import _lanes
    from qbopt.backend.peephole import _register_effects

    mine, theirs = _lanes(out_of.register), _lanes(into.register)
    for at in range(position - 1, -1, -1):
        one = block.insns[at]
        what = one.what
        if what is None or one.clobbers or one.symbol is True or one.group is not None:
            return None
        if liveness._terminator(what) or what.op is ir.Operation.BARRIER:
            return None
        effects = _register_effects(one, flags=True)
        if effects is None:
            return None
        reads, writes = effects
        # Z dead here, or the rename overwrites a live value.
        if not theirs <= after[id(one)]:
            return None
        if what.op is ir.Operation.MOVE and one.spill_store:
            return None
        if _writes(what, mine):
            # It must own the whole of Y: a partial write leaves lanes
            # belonging to some earlier definition, and renaming this one
            # alone would split the value in two.
            if not mine <= writes:
                return None
            # And it must not read Z: the rename would merge Z's value into
            # Y's operand, and a tied producer's relocated copy overwrites it.
            if reads & theirs:
                return None
            return at, bool(reads & mine)
        if reads & mine or writes & theirs:
            return None
    return None


def _writes(what: ir.Semantics, lanes: set) -> bool:
    """Whether this operation names those lanes as its own destination."""
    from qbopt.backend.peephole import _lanes

    return any(isinstance(dest, ir.Reg) and _lanes(dest.register) & lanes for dest in what.dests)


def _renamed(one: lir.Insn, before, after, result_only: bool = False) -> "lir.Insn | None":
    """`one` computing into `after` instead, or None where it cannot.

    `result_only` is Watcom's `ChangeIns(oth,Z,&oth->result)` against its
    `CantChange(&oth,Y,Z)`: a definition that never read Y only needs its
    destination moved, and rewriting its sources too would change what it
    reads.

    The encodability re-check is Watcom's, and it is not a formality: the
    substitution can name an operand the machine has no form for, and
    `select.emit` answering None is exactly `FindGenEntry` returning
    `G_UNKNOWN` there.
    """
    from qbopt.backend import select
    from qbopt.backend.peephole import _lanes
    from qbopt.backend.peephole import _register_effects
    from qbopt.backend.peephole import _register_operand

    what = one.what
    changed = replace(
        what,
        dests=tuple(_register_operand(dest, before, after) for dest in what.dests),
        sources=what.sources
        if result_only
        else tuple(_register_operand(source, before, after) for source in what.sources),
    )
    if changed == what:
        return None
    if select.emit(changed) is None:
        return None
    # Encodable is not renamed: an operand the instruction fixes -- `idiv`'s
    # EDX -- emits the same bytes under any name. The decoded effects have to
    # move from `before` to `after`, or the rename exists only in the LIR.
    renamed = replace(one, what=changed)
    was, now = _register_effects(one), _register_effects(renamed)
    if was is None or now is None:
        return None
    mine, theirs = _lanes(before), _lanes(after)
    reads = was[0] if result_only or not was[0] & mine else (was[0] - mine) | theirs
    if now != (reads, (was[1] - mine) | theirs):
        return None
    return renamed
