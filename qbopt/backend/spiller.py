"""Spilling: a value the allocator would not keep, kept in memory instead.

LLVM's `InlineSpiller`. Choosing to spill is the allocator's; making the
program work afterwards is this pass's, and until it existed the choice was
made and nothing acted on it -- `allocate.Spilled` was raised on 237 of 487
objects because an operand still named a value with no register.

Every definition of a spilled value becomes a store into its frame slot,
and every use a load out of it into a fresh value that lives only across
that one instruction. LLVM calls the fresh value the reload's, and it is
what makes the spilled value's live range vanish: nothing is live between
the store and the load, so the register the value wanted is free.

Like LLVM's InlineSpiller, an untied arithmetic source may read its slot
directly. This avoids inventing a short reload interval and another register
requirement while implementing an allocation decision, not an optimization
over LIR values.
"""

from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import target
from qbopt.backend import frame as frames
from qbopt.objectfile.module import Space
from qbopt.model.passes import LIRTransform
from qbopt.analysis.regions import addresses
from qbopt.analysis import intervals as ranges


class Spiller(LIRTransform):
    name = "spill"

    def __init__(self, spilled: "frozenset[int]", frame: "frames.Frame | None" = None) -> None:
        self.spilled = spilled
        self.frame = frame

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return spilled(body, self.spilled, self.frame)


def spilled(
    body: lir.LirBody, values: "frozenset[int]", frame: "frames.Frame | None" = None
) -> "tuple[lir.LirBody, frozenset[int]]":
    """`body` with each of `values` living in a frame slot, and the reloads.

    The second half matters as much as the first. A reload's value is live
    across one instruction and *must* have a register: spilling it again
    puts a load in front of a load and the allocator never settles -- three
    values spilled every round, three instructions added every round, for
    ever. LLVM says so as `LiveInterval::markNotSpillable`, and the caller
    says it here by handing these back to `allocate` as unspillable.
    """
    if not values:
        return body, frozenset()
    frame = frame if frame is not None else frames.of(body)
    fresh = _next_value(body)
    made: set[int] = set()
    constants = _constants(body, values)
    frame_loads = {**_stable_loads(body, values), **_frame_loads(body, values)}
    frame_homes = _frame_homes(body, values - frame_loads.keys())
    rebuilt = {**frame_loads, **{value: home for value, (home, _at) in frame_homes.items()}}
    stored = values - constants.keys() - frame_loads.keys() - frame_homes.keys()
    # Before any cell names a slot: one made at the first use's width is
    # outgrown by a wider use later, which then writes over its neighbour.
    _color_slots(body, stored, _widest(body, stored), frame)
    abandoned: set[int] = set()
    rematerialized_definitions: set[int] = set()
    identities: set[int] = set()

    blocks = []
    for block in body.blocks:
        insns: list[lir.Insn] = []
        for one in block.insns:
            if _identity(one, stored, frame):
                identities.add(id(one))
                insns.append(one)
                continue
            source = _group_source(one)
            if source is not None and source.value in constants:
                one = replace(one, what=replace(one.what, sources=(constants[source.value],)), uses=(), symbol=False)
            # A rebuilt value's cell reads as well as a slot does. Only the
            # read: `_in_place` and `_tied` write the cell back, which a
            # program's own variable is not there for. Without this the
            # rebuild was a whole instruction more than the spill it replaced
            # -- `mov si,[bp-2Eh]; add dx,si` where BC writes `add dx,[bp-2Eh]`.
            if rebuilt:
                folded = _source(one, frozenset(rebuilt), _Cells(rebuilt))
                if folded is not None:
                    one = folded
            remade = {}
            for value in one.uses:
                if (
                    (value not in constants and value not in frame_loads and value not in frame_homes)
                    or value in remade
                    or value in frame_homes
                    and id(one) == frame_homes[value][1]
                ):
                    continue
                remade[value] = fresh
                if value in constants:
                    constant = constants[value]
                    insns.append(
                        replace(
                            _inserted(
                                one,
                                ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(fresh, constant.width),), (constant,)),
                                (fresh,),
                                (),
                            ),
                            rematerialized=True,
                        )
                    )
                elif value in frame_loads:
                    insns.append(replace(_reload(one, fresh, frame_loads[value]), rematerialized=True))
                else:
                    insns.append(replace(_reload(one, fresh, frame_homes[value][0]), rematerialized=True))
                made.add(fresh)
                fresh += 1
            if remade:
                one = _renamed(one, remade)
            one = _source(one, stored, frame) or one
            direct = _in_place(one, stored, frame) or _tied(one, stored, frame)
            if direct is not None:
                insns.append(direct)
                continue
            loaded = _memory_source_read_first(one, stored, fresh)
            if loaded is not None:
                before, one = loaded
                fresh += 1
                insns.append(before)
                direct = _tied(one, stored, frame)
                if direct is not None:
                    insns.append(direct)
                    continue
            before, after, rename = [], [], {}
            for value in one.uses:
                if value not in stored or value in rename:
                    continue
                rename[value] = fresh
                before.append(_reload(one, fresh, frame.cell(value, _width(one, value))))
                fresh += 1
            for value in one.defines:
                if value not in stored:
                    continue
                if value not in rename:
                    rename[value] = fresh
                    fresh += 1
                if value not in constants:
                    after.append(_store(one, rename[value], frame.cell(value, _width(one, value))))
            insns += before
            rewritten = _renamed(one, rename) if rename else one
            insns.append(rewritten)
            if any(value in frame_loads for value in one.defines):
                rematerialized_definitions.add(id(rewritten))
            if (
                len(one.defines) == 1
                and one.defines[0] in constants
                and not (one.requires or one.delivers or one.clobbers)
                and one.group is None
                and one.symbol is not True
            ):
                abandoned.add(id(rewritten))
            insns += after
            made.update(rename.values())
        blocks.append(replace(block, insns=tuple(insns)))
    result = replace(body, blocks=tuple(blocks))
    if rematerialized_definitions:
        result = replace(
            result,
            blocks=tuple(
                replace(
                    block,
                    insns=tuple(lir.without(block.insns, lambda one: id(one) in rematerialized_definitions)),
                )
                for block in result.blocks
            ),
        )
    if identities:
        result = replace(
            result,
            blocks=tuple(
                replace(block, insns=tuple(lir.without(block.insns, lambda one: id(one) in identities)))
                for block in result.blocks
            ),
        )
    result = _remove_abandoned(result, abandoned)
    surviving = {value for one in result.insns for value in one.defines}
    return result, frozenset(made & surviving)


def _identity(one: lir.Insn, stored: "frozenset[int]", frame) -> bool:
    """A move between two spilled values that share one slot."""
    pair = _plain_move(one)
    if pair is None or not set(pair) <= stored or pair[0] == pair[1]:
        return False
    slots = getattr(frame, "slots", {})
    return pair[0] in slots and slots.get(pair[0]) == slots.get(pair[1])


def _plain_move(one: lir.Insn) -> "tuple[int, int] | None":
    what = one.what
    if (
        what is None
        or what.op is not ir.Operation.MOVE
        or len(what.dests) != 1
        or len(what.sources) != 1
        or not isinstance(what.dests[0], ir.Held)
        or not isinstance(what.sources[0], ir.Held)
        or what.dests[0].width != what.sources[0].width
        or one.requires
        or one.delivers
    ):
        return None
    return what.dests[0].value, what.sources[0].value


def siblings(body: lir.LirBody, values: "frozenset[int]", frame, fixed: "frozenset[int]") -> frozenset[int]:
    """Values copied to and from a spilled one that are cheaper in its slot.

    LLVM's InlineSpiller spills a value's siblings with it. A phi's copies
    join the accumulator it carries to the sum the loop writes; spill the
    phi alone and every pass reloads it on the edge that skips the sum and
    stores it back at the latch -- nbody's inner loop did both for accX and
    accY. In one slot, a copy between two of them writes a cell from
    itself and goes, and the sum is an `add` to the cell.

    A copy web member joins when the loop-weighted moves it removes outweigh
    the reloads and stores it adds, and when it interferes with no value
    already in the slot. A move out of the web stays a move, and an update
    that reads and writes it happens in the slot. Each web gets one slot,
    handed out here, so the spiller sees the copies as identities.
    """
    if frame is None or not values:
        return frozenset()
    from qbopt.backend import coalesce
    from qbopt.analysis import intervals

    adjacent: dict[int, set[int]] = {}
    for one in body.insns:
        pair = _plain_move(one)
        if pair is None or pair[0] == pair[1]:
            continue
        adjacent.setdefault(pair[0], set()).add(pair[1])
        adjacent.setdefault(pair[1], set()).add(pair[0])
    if not any(one in adjacent for one in values):
        return frozenset()
    # A shared slot holds each member at every width it is used, not just moved.
    widths = _widest(body, set(adjacent))

    near = coalesce._interference(body)
    deep = intervals.depths(body)
    wanted = set(adjacent)
    occurs: dict[int, list[tuple[float, lir.Insn]]] = {}
    for block in body.blocks:
        each = float(intervals.PER_LEVEL ** deep.get(block.at, 0))
        for one in block.insns:
            for value in wanted.intersection((*one.defines, *one.uses)):
                occurs.setdefault(value, []).append((each, one))

    def worth(candidate: int, group: set[int]) -> bool:
        saved = cost = 0.0
        for each, one in occurs.get(candidate, ()):
            pair = _plain_move(one)
            if pair is not None:
                if set(pair) - {candidate} <= group:
                    saved += each
                continue
            what = one.what
            in_place = (
                what is not None
                and what.op in (ir.Operation.BINARY, ir.Operation.UNARY)
                and candidate in one.defines
                and candidate in one.uses
                and not one.requires
                and not one.delivers
                and not any(isinstance(x, ir.Mem) for x in (*what.dests, *what.sources))
            )
            if not in_place:
                cost += each
        return saved > cost

    taken: set[int] = set()
    chosen: set[int] = set()
    for first in sorted(one for one in values if one in adjacent):
        if first in taken:
            continue
        group = {first}
        growing = True
        while growing:
            growing = False
            frontier = set().union(*(adjacent.get(one, set()) for one in group)) - group - taken - fixed
            for candidate in sorted(frontier):
                if widths.get(candidate) != widths.get(first):
                    continue
                if any(one in near.get(candidate, ()) for one in group):
                    continue
                if candidate in values or worth(candidate, group):
                    group.add(candidate)
                    growing = True
        slots = {frame.slots[one] for one in group if one in frame.slots}
        if len(group) < 2 or len(slots) > 1:
            continue
        home = slots.pop() if slots else frame.slot(first, widths[first])
        for one in group:
            frame.slots[one] = home
        taken |= group
        chosen |= group - values
    return frozenset(chosen)


def _color_slots(
    body: lir.LirBody, values: "set[int] | frozenset[int]", widths: dict[int, int], frame: frames.Frame
) -> None:
    """Assign compatible noninterfering spill values to the same frame slot.

    Values already assigned by an earlier spill round retain their slots: the
    rewritten body no longer carries their original live interval. New values
    are considered widest first, so a smaller value can safely occupy a larger
    slot without growing into an already allocated neighbour.
    """
    live = ranges.intervals(body)
    colors: list[tuple[int, int, list[ranges.Interval]]] = []
    pending = sorted(
        (value for value in values if value not in frame.slots),
        key=lambda value: (-max(widths[value], frames.WORD), value),
    )
    for value in pending:
        width = widths[value]
        capacity = max(width, frames.WORD)
        interval = live.get(value)
        if interval is None:
            frame.slot(value, width)
            continue
        color = next(
            (
                one
                for one in colors
                if one[1] >= capacity
                and all(not interval.overlaps(other) for other in one[2])
            ),
            None,
        )
        if color is None:
            home = frame.slot(value, width)
            colors.append((home, capacity, [interval]))
            continue
        home, _capacity, occupants = color
        frame.slots[value] = home
        occupants.append(interval)


def rematerializable(body: lir.LirBody, values: frozenset[int]) -> frozenset[int]:
    """Spill candidates whose value can be reconstructed without a slot."""
    return (
        frozenset(_constants(body, values))
        | frame_rematerializable(body, values)
        | frozenset(_stable_loads(body, values))
        | frozenset(_frame_homes(body, values))
    )


def frame_rematerializable(body: lir.LirBody, values: frozenset[int]) -> frozenset[int]:
    """Frame-loaded selectors whose proof permits eager rematerialization."""
    return frozenset(_frame_loads(body, values))


def _stable_loads(body: lir.LirBody, values: frozenset[int]) -> dict[int, ir.Mem]:
    """Values loaded from a cell nothing changes before they are used again.

    Reading the cell a second time is the same value, so the load is what a
    slot would be -- without the store, and with the read foldable into the
    instruction that wanted it. This is what makes hoisting a loop-invariant
    load free when the allocator then refuses it a register: the load moves
    back to where it was rather than becoming a store and a slot.

    `hoist` and this ask the same question, so anything `hoist` moved out of a
    loop is answerable here; the analysis is redone on LIR because spilling is
    about this body and not about the one the MIR pass saw.
    """
    if not values:
        return {}
    definitions: dict[int, list[tuple[lir.Insn, ir.Mem]]] = {}
    uses: dict[int, list[lir.Insn]] = {value: [] for value in values}
    for block in body.blocks:
        for one in block.insns:
            for value in values.intersection(one.uses):
                uses[value].append(one)
            for value in values.intersection(one.defines):
                cell = None
                match one.what:
                    case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held() as dest,), (ir.Mem() as source,)):
                        if (
                            dest.value == value
                            and dest.width == source.width
                            and source.addr is not None
                            and one.defines == (value,)
                            and not one.uses
                            and not one.clobbers
                            and one.group is None
                            and not one.spread
                            and one.symbol is not True
                        ):
                            cell = source
                definitions.setdefault(value, []).append((one, cell))

    result: dict[int, ir.Mem] = {}
    for value in values:
        found = definitions.get(value)
        if not found or len(found) != 1 or found[0][1] is None or not uses[value]:
            continue
        define, cell = found[0]
        if any(one.group is not None for one in uses[value]):
            continue
        if _unchanged(body, define, cell, uses[value]):
            result[value] = cell
    return result


def _keeps(one: lir.Insn, define: lir.Insn, cell: ir.Mem, holds: bool) -> bool:
    """Whether `cell` still holds what it held at `define` after `one`."""
    if one is define:
        return True
    written = _written(one, cell)
    return (
        holds
        and not _may_write(one.op, cell)
        and not any(dest.addr is None or addresses(cell.addr, cell.width, dest.addr, dest.width) for dest in written)
    )


def _may_write(op: "mir.Op | None", cell: ir.Mem) -> bool:
    """Whether the MIR operation may change `cell`: through what it says it
    stores, and anywhere at all where that is not stated. A call stating a
    store with no address that spares the whole frame leaves a frame cell alone."""
    from qbopt.analysis import effects

    if op is None:
        return False
    if effects.unmodeled_write(op):
        return True
    for ref in op.stores:
        if _in_frame(cell) and mir.WHOLE_FRAME in ref.excludes:
            continue
        if ref.addr is None or addresses(cell.addr, cell.width, ref.addr, ref.width):
            return True
    return False


def _in_frame(cell: ir.Mem) -> bool:
    return cell.addr is not None and cell.addr.space is Space.FRAME


def _written(one: lir.Insn, cell: ir.Mem) -> list[ir.Mem]:
    """The memory this instruction names as written that could be `cell`.

    Outside the frame, a destination is what the MIR operation stores, so
    stores that spare the whole frame answer for it: `mov es:[bx+10],ax`
    cannot write a parameter whose frame nothing lets escape."""
    op = one.op
    spared = (
        _in_frame(cell)
        and op is not None
        and bool(op.stores)
        and all(mir.WHOLE_FRAME in ref.excludes for ref in op.stores)
    )
    return [
        dest
        for dest in (one.what.dests if one.what is not None else ())
        if isinstance(dest, ir.Mem) and not (spared and dest.addr is not None and dest.addr.space is not Space.FRAME)
    ]


def _unchanged(body: lir.LirBody, define: lir.Insn, cell: ir.Mem, uses: list) -> bool:
    """Whether every use of the loaded value sees the cell the load saw."""
    predecessors = {block.at: [] for block in body.blocks}
    for block in body.blocks:
        for at in block.succ:
            if at in predecessors:
                predecessors[at].append(block.at)
    blocks = {block.at: block for block in body.blocks}
    into = {at: at != body.entry for at in blocks}
    outof: dict[int, bool] = {}
    changing = True
    while changing:
        changing = False
        for at, block in blocks.items():
            holds = into[at]
            for one in block.insns:
                holds = _keeps(one, define, cell, holds)
            if outof.get(at) != holds:
                outof[at] = holds
                changing = True
        for at in blocks:
            if at == body.entry or not predecessors[at]:
                continue
            met = all(outof.get(parent, True) for parent in predecessors[at])
            if met != into[at]:
                into[at] = met
                changing = True

    wanted = {id(one) for one in uses}
    for block in body.blocks:
        holds = into[block.at]
        for one in block.insns:
            if id(one) in wanted and not holds:
                return False
            holds = _keeps(one, define, cell, holds)
    return True


def _frame_loads(body: lir.LirBody, values: frozenset[int]) -> dict[int, ir.Mem]:
    """Stable native arguments that may be loaded again at each use.

    This deliberately recognizes only word selectors loaded from a positive
    BP-relative argument slot.  A local can change, an exposed address can be
    changed through an alias, and a general memory load may observe a call or
    a store.  All uses must be later in the defining block, with no call,
    incomplete memory barrier, or store between the definition and the last
    use.  That is enough for CSE'd selector reloads while making no claim about
    arbitrary load rematerialization.
    """
    pinned = {getattr(value, "id", value): register for value, register in body.pins.items()}
    candidates = {value for value in values if pinned.get(value) in target.SEGMENTS}
    if not candidates:
        return {}

    definitions: dict[int, tuple[int, int, lir.Insn, ir.Mem]] = {}
    uses: dict[int, list[tuple[int, int]]] = {value: [] for value in candidates}
    excluded: set[int] = set()
    for block_index, block in enumerate(body.blocks):
        for insn_index, one in enumerate(block.insns):
            for value in candidates.intersection(one.uses):
                uses[value].append((block_index, insn_index))
            for value in candidates.intersection(one.defines):
                match one.what:
                    case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held() as dest,), (ir.Mem() as source,)):
                        stable = (
                            dest.value == value
                            and dest.width == source.width == 2
                            and source.addr is not None
                            and source.addr.space is Space.FRAME
                            and source.addr.disp > 0
                            and source.base is None
                            and source.through == Register.BP
                            and one.defines == (value,)
                            and not one.uses
                        )
                    case _:
                        stable = False
                if not stable or value in definitions:
                    excluded.add(value)
                else:
                    definitions[value] = (block_index, insn_index, one, source)

    result = {}
    for value in candidates - excluded:
        definition = definitions.get(value)
        locations = uses[value]
        if definition is None or not locations:
            continue
        block_index, defined_at, _one, source = definition
        if any(use_block != block_index or used_at <= defined_at for use_block, used_at in locations):
            continue
        last_use = max(used_at for _block, used_at in locations)
        safe = True
        for one in body.blocks[block_index].insns[defined_at + 1 : last_use + 1]:
            if _may_write(one.op, source):
                safe = False
                break
        if safe:
            result[value] = source
    return result


def _frame_homes(body: lir.LirBody, values: frozenset[int]) -> dict[int, tuple[ir.Mem, int]]:
    """Existing stable frame stores which can hold a spilled SSA value.

    Native C frequently copies an aggregate field into a local and continues
    using the SSA value as well, and a loop counter is stored to its own local
    every iteration.  Giving that value a second frame slot makes the allocator
    store it twice.  The original local is already its home wherever the store
    is the last thing to have written the cell -- which is a fact about a point
    in the program and not about the body, so a call the store runs after does
    not disqualify the cell.  Asking it of the body was what left deedlines'
    plasmablobs storing `x%` to a slot of its own beside its own `[bp-2Eh]`.
    Return the store's identity as well as the cell so spilling preserves the
    one use which initializes the home.
    """
    if not values:
        return {}
    definitions: dict[int, list[tuple[int, int]]] = {value: [] for value in values}
    uses: dict[int, list[tuple[int, int, lir.Insn]]] = {value: [] for value in values}
    candidates: dict[int, list[tuple[int, int, lir.Insn, ir.Mem]]] = {value: [] for value in values}

    for block_index, block in enumerate(body.blocks):
        for insn_index, one in enumerate(block.insns):
            for value in values.intersection(one.defines):
                definitions[value].append((block_index, insn_index))
            for value in values.intersection(one.uses):
                uses[value].append((block_index, insn_index, one))
            match one.what:
                case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Mem() as dest,), (ir.Held() as source,)):
                    original = one.covers is not None and one.covers[0] != one.covers[1]
                    if (
                        source.value in values
                        and dest.width == source.width
                        and dest.addr is not None
                        and dest.addr.space is Space.FRAME
                        and dest.addr.disp < 0
                        and dest.base is None
                        and dest.through == Register.BP
                        and not dest.stack_argument
                        and original
                        and one.group is None
                    ):
                        candidates[source.value].append((block_index, insn_index, one, dest))
                case _:
                    pass

    result: dict[int, tuple[ir.Mem, int]] = {}
    for value in values:
        homes = candidates[value]
        # The store reads the definition kept in a register; any other is
        # renamed to a reload and never reaches the home.
        if not homes or len(definitions[value]) != 1:
            continue
        eligible = []
        for _block, _index, store, home in homes:
            later = [site for site in uses[value] if site[2] is not store]
            if not later or any(one.group is not None for _b, _i, one in later):
                continue
            if _home_holds(body, value, home, store):
                eligible.append((home, id(store)))
        if len(eligible) == 1:
            result[value] = eligible[0]
    return result


def _holding(one: lir.Insn, value: int, home: ir.Mem, store: lir.Insn, holds: bool) -> bool:
    """Whether `home` still holds `value` after `one`."""
    if one is store:
        return True
    if value in one.defines:
        return False
    written = _written(one, home)
    return (
        holds
        and not _may_write(one.op, home)
        and not any(
            cell is not home and (cell.addr is None or addresses(home.addr, home.width, cell.addr, cell.width))
            for cell in written
        )
    )


def _home_holds(body: lir.LirBody, value: int, home: ir.Mem, store: lir.Insn) -> bool:
    """Whether `home` holds `value` at every use of it but the store itself."""
    predecessors = {block.at: [] for block in body.blocks}
    for block in body.blocks:
        for at in block.succ:
            if at in predecessors:
                predecessors[at].append(block.at)
    blocks = {block.at: block for block in body.blocks}
    # A must-analysis: unreached is "holds", so a loop header is not told by
    # its own backedge that the fact fails before the fixpoint has run.
    into = {at: at != body.entry for at in blocks}
    outof: dict[int, bool] = {}
    changing = True
    while changing:
        changing = False
        for at, block in blocks.items():
            holds = into[at]
            for one in block.insns:
                holds = _holding(one, value, home, store, holds)
            if outof.get(at) != holds:
                outof[at] = holds
                changing = True
        for at in blocks:
            if at == body.entry or not predecessors[at]:
                continue
            met = all(outof.get(parent, True) for parent in predecessors[at])
            if met != into[at]:
                into[at] = met
                changing = True

    for block in body.blocks:
        holds = into[block.at]
        for one in block.insns:
            if value in one.uses and one is not store and not holds:
                return False
            holds = _holding(one, value, home, store, holds)
    return True


def _remove_abandoned(body: lir.LirBody, abandoned: set[int]) -> lir.LirBody:
    if not abandoned:
        return body
    # BARRIER is not an excuse for an unrepresented machine use. Lowering
    # records every value a recognized opaque instruction reads in `uses`
    # and every fixed-register occurrence in `requires`; an instruction for
    # which that cannot be done must be refused before allocation. Treating
    # the mere presence of any barrier anywhere in the body as a hidden use
    # kept an unrelated, now-dead `mov ax,27` alive in OIMAD. Six call-result
    # ranges already occupied all six GP registers at that point, so the
    # allocator selected the dead literal forever instead of turning its
    # original byte span into an inert anchor.
    used = {value for one in body.insns for value in one.uses}
    used.update(held.value for one in body.insns for held, _ in one.requires)
    used.update(value for block in body.blocks for value in block.arrives)
    used.update(value for block in body.blocks for phi in block.phis for _, value in phi.incoming)

    def removable(one: lir.Insn) -> bool:
        return id(one) in abandoned and not used.intersection(one.defines)

    def anchor(one: lir.Insn) -> lir.Insn:
        if not removable(one):
            return one
        return replace(one, what=ir.Semantics(ir.Operation.NOTHING, "nop"), defines=(), uses=(), widths=())

    return replace(
        body,
        blocks=tuple(
            replace(block, insns=tuple(anchor(one) for one in lir.without(block.insns, removable)))
            for block in body.blocks
        ),
    )


class _Cells:
    """Where a rebuilt value already is, in the shape `_source` asks a frame."""

    def __init__(self, cells: "dict[int, ir.Mem]") -> None:
        self._cells = cells

    def cell(self, value: int, width: int) -> "ir.Mem | None":
        found = self._cells.get(value)
        return found if found is not None and found.width == width else None


def folded_source(one: lir.Insn, values: frozenset[int]) -> "ir.Held | None":
    """The spilled source arithmetic or a comparison reads as its memory operand, needing no reload."""
    if one.group is not None or one.requires or one.delivers or one.clobbers:
        return None
    match one.what:
        case ir.Semantics(ir.Operation.BINARY, name, (ir.Held() as dest,), (ir.Held() as left, ir.Held() as right)):
            if name not in {"add", "sub", "and", "or", "xor"} or dest != left:
                return None
        case ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ir.Held() as left, ir.Held() as right)):
            pass
        case _:
            return None
    if (
        left.width not in (2, 4)
        or right.width != left.width
        or right.value not in values
        or left.value == right.value
        or right.value in one.defines
        or any(value in values and value not in {left.value, right.value} for value in one.uses)
    ):
        return None
    return right


def _source(one, values, frame):
    """Fold one untied spill source into arithmetic or a comparison."""
    right = folded_source(one, values)
    if right is None:
        return None
    left = one.what.sources[0]
    cell = frame.cell(right.value, right.width)
    if cell is None:
        return None
    return replace(
        one,
        what=replace(one.what, sources=(left, cell)),
        symbol=False,
        uses=tuple(value for value in one.uses if value != right.value),
    )


def _group_source(one):
    """An unconstrained full-width parallel copy can take a literal in place."""
    if one.group is None or one.clobbers or one.requires or one.delivers:
        return None
    match one.what:
        case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held() as dest,), (ir.Held() as source,)):
            if dest.width == source.width and one.uses == (source.value,) and one.defines == (dest.value,):
                return source
    return None


def _constants(body: lir.LirBody, values: frozenset[int]) -> dict[int, ir.Imm]:
    """Literal values, including full-width copies with one unambiguous definition."""
    definitions: dict[int, list[lir.Insn]] = {}
    excluded = set()
    widths = {}
    for one in body.insns:
        for value in one.defines:
            definitions.setdefault(value, []).append(one)
        for value in one.uses:
            widths[value] = max(widths.get(value, 0), _width(one, value))
        if one.group is not None:
            excluded.update(one.defines)
            if _group_source(one) is None:
                excluded.update(one.uses)
    sources = {}
    for value in definitions.keys() - excluded:
        defining = definitions.get(value, [])
        if len(defining) != 1:
            continue
        one = defining[0]
        what = one.what
        if (
            what is None
            or what.op is not ir.Operation.MOVE
            or len(what.dests) != 1
            or len(what.sources) != 1
            or one.defines != (value,)
            or one.clobbers
        ):
            continue
        into, source = what.dests[0], what.sources[0]
        if (
            isinstance(into, ir.Held)
            and isinstance(source, (ir.Imm, ir.Held))
            and into.width == source.width
            and one.uses == ((source.value,) if isinstance(source, ir.Held) else ())
        ):
            if widths.get(value, 0) <= source.width:
                sources[value] = source
    result = {}
    while True:
        before = len(result)
        for value, source in sources.items():
            constant = result.get(source.value) if isinstance(source, ir.Held) else source
            if constant is not None:
                result[value] = constant
        if len(result) == before:
            return {value: constant for value, constant in result.items() if value in values}


def _widest(body: lir.LirBody, values) -> dict[int, int]:
    """How wide each value is read or written anywhere, which is how big its slot has to be."""
    widths: dict[int, int] = {}
    for one in body.insns:
        for value in values & {*one.defines, *one.uses}:
            widths[value] = max(widths.get(value, 0), _width(one, value))
    return widths


def _width(one: lir.Insn, value: int) -> int:
    """How wide this instruction reads or writes the value, defaulting to a word.

    A requirement answers for an operation that names no operand at all:
    the restore idiom pushes four bytes and its semantics have no
    destination to say so, so the reload came back two bytes wide and the
    push carried a stale high half.
    """
    for held, _register in one.requires + one.delivers:
        if held.value == value:
            return held.width
    for named, width in one.widths:
        if named == value:
            return width
    if one.what is None:
        return frames.WORD
    # A cell's base and index too: `[eax+edx*2]` reads all of edx.
    for where in (*one.what.dests, *one.what.sources):
        for held in ir.values(where):
            if held.value == value:
                return held.width
    return frames.WORD


def _reload(beside: lir.Insn, into: int, cell: ir.Mem) -> lir.Insn:
    """The load that puts a spilled value back for one instruction."""
    return replace(
        _inserted(beside, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(into, cell.width),), (cell,)), (into,), ()),
        spill_reload=True,
    )


def _store(beside: lir.Insn, out_of: int, cell: ir.Mem) -> lir.Insn:
    """The store that puts a spilled value away as soon as it is written."""
    return replace(
        _inserted(
            beside, ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (ir.Held(out_of, cell.width),)), (), (out_of,)
        ),
        spill_store=True,
    )


def _inserted(beside: lir.Insn, what: ir.Semantics, defines: tuple, uses: tuple) -> lir.Insn:
    """An instruction that stands beside another and claims none of its bytes."""
    at = beside.covers[0] if beside.covers else beside.at
    return lir.Insn(at=beside.at, covers=(at, at), what=what, defines=defines, uses=uses, op=beside.op)


def _wants(side: tuple, rename: dict[int, int]) -> tuple:
    """A requirement, naming whichever value now feeds the instruction.

    The register an instruction demands is a fact about the instruction,
    so replacing the value that reaches it does not lift the demand. Left
    naming the value that was spilled, the restore idiom's reload arrived
    unpinned and the allocation put it in cx while `push eax` went on
    reading eax.
    """
    return tuple((ir.Held(rename.get(held.value, held.value), held.width), register) for held, register in side)


def _renamed(one: lir.Insn, rename: dict[int, int]) -> lir.Insn:
    """The instruction reading and writing the reload's value instead."""
    what = one.what
    if what is None:
        return replace(
            one,
            defines=tuple(rename.get(v, v) for v in one.defines),
            uses=tuple(rename.get(v, v) for v in one.uses),
            requires=_wants(one.requires, rename),
            delivers=_wants(one.delivers, rename),
            widths=tuple((rename.get(value, value), width) for value, width in one.widths),
        )
    return replace(
        one,
        what=replace(
            what,
            dests=tuple(_settled(x, rename) for x in what.dests),
            sources=tuple(_settled(x, rename) for x in what.sources),
        ),
        defines=tuple(rename.get(v, v) for v in one.defines),
        uses=tuple(rename.get(v, v) for v in one.uses),
        requires=_wants(one.requires, rename),
        delivers=_wants(one.delivers, rename),
        widths=tuple((rename.get(value, value), width) for value, width in one.widths),
    )


def _settled(where, rename: dict[int, int]):
    """One operand with every value it names put through the rename.

    Through `ir.mapped`, so a cell's base is renamed with the rest: this
    looked in `Mem.through`, which holds a register, and spilling a
    pointer renamed the load's `uses` and left the cell addressed through
    a value that now lives in a frame slot.
    """
    return ir.mapped(where, lambda one: ir.Held(rename.get(one.value, one.value), one.width))


class Simultaneous(Exception):
    """A move in a parallel copy with both ends spilled.

    `mov [bp-2],[bp-4]` is not an instruction, and breaking it into a load
    and a store puts an ungrouped one inside a copy whose moves happen at
    once -- which is what parcopy.py then cannot schedule as one group.
    Retained for callers reporting this legacy refusal. Frame-to-frame
    copies now stay grouped until parcopy expands them after scheduling.
    """


def _in_place(one: lir.Insn, values: "frozenset[int]", frame) -> "lir.Insn | None":
    """One move of a parallel copy, with its spilled end read or written where it lives.

    A phi's moves happen at once. Spilling one of them the ordinary way --
    a reload before it and a store after it -- puts an instruction inside
    the group that is not part of it, and the group stops being one run.
    The slots go in the operands and the copy stays one instruction until
    parcopy has ordered the whole group. It then expands memory-to-memory
    copies to balanced stack transfers.
    """
    what = one.what
    if one.group is None or what is None or what.op is not ir.Operation.MOVE:
        return None
    if len(what.dests) != 1 or len(what.sources) != 1:
        return None
    into = [v for v in one.defines if v in values]
    outof = [v for v in one.uses if v in values]
    if not into and not outof:
        return None
    if into and outof:
        return replace(
            one,
            what=replace(
                what,
                dests=(frame.cell(into[0], _width(one, into[0])),),
                sources=(frame.cell(outof[0], _width(one, outof[0])),),
            ),
            defines=tuple(value for value in one.defines if value not in into),
            uses=tuple(value for value in one.uses if value not in outof),
        )
    value = (into or outof)[0]
    cell = frame.cell(value, _width(one, value))
    if into:
        return replace(
            one,
            what=replace(what, dests=(cell,)),
            defines=tuple(v for v in one.defines if v != value),
        )
    return replace(
        one,
        what=replace(what, sources=(cell,)),
        uses=tuple(v for v in one.uses if v != value),
    )


def _memory_source_read_first(
    one: lir.Insn, values: "frozenset[int]", fresh: int
) -> "tuple[lir.Insn, lir.Insn] | None":
    """The load, and the instruction reading the loaded value instead.

    Only for a tie: a value an instruction both reads and writes has no
    remedy but the operand, and that is unavailable while another operand
    is already a cell. Reading the cell into a value first leaves the one
    memory operand for the tie. The load is short and the allocator places
    it like any other reload.
    """
    what = one.what
    if what is None or one.group is not None:
        return None
    tied = [value for value in one.defines if value in values and value in one.uses]
    if len(tied) != 1:
        return None
    cells = [x for x in what.sources if isinstance(x, ir.Mem)]
    if len(cells) != 1 or any(isinstance(x, ir.Mem) for x in what.dests):
        return None
    cell = cells[0]
    held = ir.Held(fresh, cell.width)
    load = _inserted(
        one,
        ir.Semantics(ir.Operation.MOVE, "mov", (held,), (cell,)),
        (fresh,),
        tuple(where.value for where in ir.values(cell)),
    )
    # The cell goes to the load, and its fixup with it. Left on the
    # instruction that no longer reads memory, the fixup was bound to
    # whatever field it did have: lngmix, with both its divides hoisted,
    # spilled the accumulator here and the address of `a` was written over
    # the `[bp-22h]` the add kept.
    load = replace(load, symbol=True)
    return load, replace(
        one,
        symbol=False,
        what=replace(what, sources=tuple(held if x is cell else x for x in what.sources)),
        uses=tuple(one.uses) + (fresh,),
    )


def _tied(one: lir.Insn, values: "frozenset[int]", frame) -> "lir.Insn | None":
    """A value an instruction both reads and writes, kept in its slot.

    Spilling one of these buys nothing: the reload is tied at the same
    instruction, so the next round faces the same conflict with a new
    value, and the allocator adds two instructions per round for ever --
    pressx spilled 207, then 212, then 215, at one `add`.

    x86 reads and writes memory in place, so the slot goes in the operand
    on both sides and no reload or store is needed. Only where nothing
    else in the instruction wants memory: one memory operand is all an
    instruction has. What can be encoded is select.py's to say, and it is
    asked here rather than at the end: `imul r16,rm16` has no form writing
    memory, and leaving that for emission turned qbdemo into a refusal.
    """
    what = one.what
    if what is None or one.group is not None:
        return None
    tied = [v for v in one.defines if v in values and v in one.uses]
    if len(tied) != 1:
        return None
    value = tied[0]
    # The destination's read is implicit in a two-address operation;
    # a second source naming it still needs a distinct encoded operand.
    if any(isinstance(arg, ir.Held) and arg.value == value for arg in what.sources[1:]):
        return None
    # A fixed register is a fixed register: a slot is not one.
    if any(
        isinstance(side[where.index], ir.Held) and side[where.index].value == value
        for where in target.requirements(what)
        for side in ((what.dests if where.side == "dest" else what.sources),)
        if where.index < len(side)
    ):
        return None
    # One memory operand is all there is, so nothing else may want it.
    for operand in (*what.dests, *what.sources):
        if isinstance(operand, ir.Mem):
            return None
        if isinstance(operand, ir.Held) and operand.value != value and operand.value in values:
            return None
    cell = frame.cell(value, _width(one, value))
    swap = lambda x: cell if isinstance(x, ir.Held) and x.value == value else x  # noqa: E731
    made = replace(
        what,
        dests=tuple(swap(x) for x in what.dests),
        sources=tuple(swap(x) for x in what.sources),
    )
    if not _encodable(made):
        return None
    return replace(
        one,
        what=made,
        defines=tuple(v for v in one.defines if v != value),
        uses=tuple(v for v in one.uses if v != value),
    )


def _encodable(what: ir.Semantics) -> bool:
    """Whether this form exists, asked of the one place that knows.

    Every value still held goes to a register of its width, distinct per
    value so two operands never collide by accident. The answer wanted is
    about the cell -- whether the operation writes memory at all -- and
    which registers the allocation ends up choosing does not change it.
    """
    from qbopt.backend import select

    taken: dict[int, object] = {}
    rows = {
        width: [one for one in target.WIDTHS if target.WIDTHS[one] == width and one in target.AVAILABLE]
        for width in (1, 2, 4)
    }

    def placed(operand):
        if not isinstance(operand, ir.Held):
            return operand
        if operand.value not in taken:
            row = rows.get(operand.width) or ()
            if len(taken) >= len(row):
                return operand  # refuses below, which is the safe answer
            taken[operand.value] = row[len(taken)]
        return ir.Reg(target.named(taken[operand.value], operand.width), operand.width)

    probe = replace(
        what,
        dests=tuple(placed(one) for one in what.dests),
        sources=tuple(placed(one) for one in what.sources),
    )
    return select.emit(probe) is not None


def _next_value(body: lir.LirBody) -> int:
    """One past the highest value id this body names."""
    seen = {0}
    for block in body.blocks:
        seen.update(block.arrives)
        for one in block.insns:
            seen.update(one.defines)
            seen.update(one.uses)
    return max(seen) + 1
