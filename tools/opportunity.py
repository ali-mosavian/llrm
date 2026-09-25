"""
What BC leaves on the table, counted rather than assumed.

This exists because the roadmap said for months that the classic passes
measure empty on BC's output, and that was the metric rather than BC. Every
measurement behind it asked a question shaped so that BC's own style could
not answer it: CSE over SSA values, when BC never recomputes and always
reloads; a redundant load defined as one into the register that already
holds the cell, when BC reloads into a different one.

So the questions here are deliberately crude and about *cells*, not values:
what is loaded that was just written, what is loaded twice, what is stored
twice with nothing reading it in between. A crude count that is right is
worth more than a precise one that is measuring the wrong thing.

Block-scoped, and that is a floor rather than an answer: BC's loop counter
round-trips through memory every iteration and the reload arrives over the
back-edge, which nothing here sees.
"""

import sys
import argparse
from pathlib import Path
from collections import Counter
from dataclasses import replace

import iced_x86

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from qbopt import rewrite
from qbopt.model import ir
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.analysis import avail
from qbopt.objectfile import omf
from qbopt.objectfile import module
from qbopt.analysis import loops as loopy
from qbopt.objectfile.module import Space
from qbopt.frontend import blocks as split
from qbopt.frontend.blocks import code_map


def _program(path: Path, records=None) -> str:
    """Use the object's source identity, independent of output-file naming.

    Headerless objects and report-only names retain the fixture convention.
    DOS source paths are normalized without depending on the host platform.
    """
    if records is None and path.is_file():
        records = omf.parse(path.read_bytes())
    for record in records or ():
        if record.type != omf.THEADR or not record.body:
            continue
        length = record.body[0]
        if length and len(record.body) >= length + 1:
            source = record.body[1 : length + 1].decode("latin-1").replace("\\", "/")
            name = Path(source).stem
            if name:
                return name.upper()
    return path.stem.split("-")[0].upper()


def named(ref) -> bool:
    """A cell this can name. A stack slot is a depth from the top of its own
    block, and an address of None aliases everything."""
    return ref.addr is not None and ref.addr.space is not Space.STACK


def _precise(ref):
    ref = mir._symbolic_ref(ref)
    return ref if named(ref) and ref.base is None and ref.segment is None and ref.width > 0 else None


def _overlap_known(first, second, dgroup):
    start = max(first.addr.disp, second.addr.disp)
    end = min(first.addr.disp + first.width, second.addr.disp + second.width)
    if start >= end:
        return False
    shared = replace(first, addr=first.addr.plus(start - first.addr.disp), width=end - start)
    return avail._covered_by(shared, second, dgroup)


def _through(block, module_, entering: dict, seen: set, found: Counter | None):
    """One block, forward, from what its predecessors agreed on.

    Returns what is known on the way out. `found` is None on the rounds that
    are only settling the fixed point -- counting them would count the same
    site once per round.
    """
    written = dict(entering)
    read = set(seen)
    for op in block.ops:
        # A call may write any cell, and a barrier's addresses are its own.
        if op.barrier or op.at in module_.calls:
            written.clear()
            read.clear()
            continue
        for ref in op.loads:
            if ref.space is Space.STACK or ref.addr is not None and ref.addr.space is Space.STACK:
                continue
            cell = _precise(ref)
            if found is not None and cell is not None:
                if any(avail._covered_by(cell, one, module_.dgroup) for one in written):
                    found["load of a cell just written"] += 1
                elif any(avail._covered_by(cell, one, module_.dgroup) for one in read):
                    found["load of a cell already loaded"] += 1
            written = {one: at for one, at in written.items() if not mir.overlapping(one, ref, module_.dgroup)}
            if cell is not None:
                read.add(cell)
        for ref in op.stores:
            if ref.space is Space.STACK or ref.addr is not None and ref.addr.space is Space.STACK:
                continue
            cell = _precise(ref)
            if found is not None and cell is not None:
                for previous in written:
                    if avail._covered_by(previous, cell, module_.dgroup):
                        found["store over a store nothing read"] += 1
                    elif _overlap_known(previous, cell, module_.dgroup):
                        found["partial overwrite of unread stored bytes"] += 1
            written = {one: at for one, at in written.items() if not mir.overlapping(one, ref, module_.dgroup)}
            read = {one for one in read if not mir.overlapping(one, ref, module_.dgroup)}
            if cell is not None:
                written[cell] = op.at
    return written, read


# What a 16-bit body has to hand: bp is the frame pointer, sp the stack, and
# the segment registers are not general. Six, and BC uses two.
GENERAL = ("AX", "BX", "CX", "DX", "SI", "DI")

# iced's Register is a bag of int constants rather than an enum, so the name
# of one has to be looked back up.
NAMED = {
    getattr(iced_x86.Register, one): one
    for one in dir(iced_x86.Register)
    if not one.startswith("_") and isinstance(getattr(iced_x86.Register, one), int)
}


# What one operation costs, in 386 cycles, near enough to rank by. A memory
# operand is counted on top of the instruction, which is the whole point:
# BC's per-statement code pays for one on almost every line.
CYCLES = {"imul": 22, "idiv": 43, "mul": 22, "div": 43, "shl": 3, "shr": 3, "sar": 3}
TOUCH = 4  # a memory operand, cached

# A call is charged for what the callee does, not for the `call`. Twenty was
# the flat rate and it undercounts the ones that matter: B$HARY takes a
# subscript as a long and returns es:bx, which is a 32-bit multiply and a
# segment normalisation -- and none of it appears in the caller's own
# instructions, which is exactly why it went unmeasured. The long arithmetic
# helpers are their own instruction's cost plus the call overhead.
CALLED = {
    "B$HARY": 150,
    "B$MUI4": 70,
    "B$DVI4": 120,
    "B$RMI4": 120,
    "B$CPI4": 40,
    # Full straight-line bodies, including waits and stack traffic; see docs/optimizations/float-integer-values.md.
    # FIL2 executes CWD and then the complete FILD body.  FILD itself saves
    # BP/BX, materializes DX:AX on its stack, executes the x87 load, restores
    # the frame and far-returns. Charging either as a bare far CALL made
    # replacing FIL2 with one `fild` look slower, even though the replacement
    # removes that whole wrapper.  These are deliberately conservative 386
    # rankings of the literal 87bhelp.asm listings named in runtime.toml.
    "B$FIL2": 90,
    "B$FILD": 88,
    "B$FIST": 106,
    "B$FIS2": 100,
}
CALL = 20

# Floating point, priced as the x87 instruction it is. BC's default is
# /FPi, which emits `int 34h`..`3Dh` where the instruction goes -- but the
# emulator patches those sites to the real opcodes at load when a
# coprocessor is present, so the cost is a 387's and not a software
# emulation's. declen.py decodes the interrupt to the instruction it stands
# in for, so the mnemonic is already the right thing to price.
#
# What is bad about BC's floating point is not the instruction selection --
# it keeps intermediates on the stack rather than spilling them, which is
# more than it manages with integers. It is that every operand is reloaded
# from memory on every pass, and a wholly invariant expression is computed
# again each time.
FLOAT = {
    "fld": 20,
    "fild": 25,
    "fst": 25,
    "fstp": 25,
    "fist": 30,
    "fistp": 30,
    "fadd": 25,
    "faddp": 25,
    "fsub": 25,
    "fsubp": 25,
    "fsubr": 25,
    "fsubrp": 25,
    "fmul": 30,
    "fmulp": 30,
    "fdiv": 90,
    "fdivp": 90,
    "fdivr": 90,
    "fdivrp": 90,
    "fcom": 20,
    "fcomp": 20,
    "fcompp": 20,
    "fchs": 10,
    "fabs": 10,
    "fsqrt": 120,
    "wait": 5,
    "fxch": 10,
    "fldz": 15,
    "fld1": 15,
}


def _reloads(body, module_, found: Counter) -> None:
    """Segment loads inside a loop, from something the loop never writes.

    The one BC hides. A dynamic array's elements are reached through its
    descriptor, so every subscript is `mov es,[desc+2]` -- twice in one
    statement if the element appears twice -- and the descriptor is written
    once, by B$DDIM, before the loop. A hundred iterations reload es a
    hundred times from a word that has not changed.

    The old implementation inspected ``Op.made`` to find the physical ES
    destination.  MIR deliberately no longer carries that machine form, so
    this report does not manufacture a segment-register count from source
    provenance.  The emitted-code quality report measures actual loads.
    """
    at_of = {block.at: block for block in body.blocks}
    # A multiply inside a loop is the induction-variable question: an
    # element's address is affine in the counter, so recomputing it with a
    # multiply on every pass is what strength reduction replaces with one
    # add. BC emits one per subscript per statement, and a two-dimensional
    # subscript makes it a multiply by the row width.
    where: set[int] = set()
    for loop in loopy.loops(list(body.blocks), body.entry):
        for at in loop.body:
            for op in at_of[at].ops:
                if op.at in where:
                    continue
                if op.kind in (mir.Kind.MUL, mir.Kind.DIVMOD):
                    where.add(op.at)
                    found["a multiply or divide inside a loop"] += 1


def _spill_cost(body, module_) -> Counter:
    """What each variable costs to leave in memory, by where it is touched.

    A variable read a hundred times in an inner loop and one read ten times
    in the outer are not the same claim on a register, and which to spill is
    decided by exactly this number. BC does not ask: it spills every
    variable at every statement boundary, so the ranking below is the whole
    of what an allocator would have to work from and none of it is used.

    Ten per level of nesting, the same stand-in for a trip count the cost
    model uses.
    """
    depth = loopy.depth(list(body.blocks), body.entry)
    out: Counter = Counter()
    for block in body.blocks:
        weight = 10 ** min(depth.get(block.at, 0), 3)
        for op in block.ops:
            if op.at in module_.calls:
                continue
            for ref in (one for one in (*op.loads, *op.stores) if named(one)):
                out[str(ref.addr)] += weight
    return out


# What each loop level actually runs, read off the program's own bounds.
# Ten per level was a stand-in and it undercounts every nest here: matrix's
# inner loop runs four hundred times, not a hundred.
TRIPS = {
    "HOTLOP": 20,
    "PRESS": 10,
    "ARRIDX": 20,
    "IVCHAN": 21,
    "STRIDE": 21,
    "MATRIX": 20,
    "NESTED": 6,
    "SPILL": 10,
    "SPLIT": 10,
    "ADDRM": 20,
    "ROTATE": 10,
    "SEGLD": 20,
    "HARR": 10,
    "HG": 10,
    "LNGMIX": 10,
    "FPCSE": 10,
    "FX": 10,
    # the opaque twins run the same loops
    "HOTLPX": 20,
    "PRESSX": 10,
    "FPCSEX": 10,
    "LNGMXX": 10,
    "JUMPS": 3,
}


def _cost(body, module_, found: Counter, trips: int = 10) -> None:
    """One number to minimise: cycles, weighted by how often a loop runs.

    Ten per level of nesting, which is a stand-in for a trip count nothing
    here knows. The point is not the absolute figure -- it is that the same
    program compiled two ways can be ranked, and that an `idiv` in an inner
    loop outranks a hundred straight-line moves, which is what BC's output
    actually costs.
    """
    mapped = code_map(module_)
    if isinstance(mapped, str):
        raise Unmeasured(mapped)
    physical = {block.at: block for block in split.partition(module_, mapped)}
    if any(block.at not in physical for block in body.blocks):
        raise Unmeasured("a costed block has no decoded instructions")
    _weighted_cost([physical[block.at] for block in body.blocks], body.entry, module_, found, trips)


def _execution_blocks(blocks, entry, contracts):
    trimmed = {}
    for block in blocks:
        for index, insn in enumerate(block.insns):
            contract = contracts.get(insn.at)
            if contract is not None and contract.established and contract.control is runtime.Control.NEVER:
                block = replace(block, insns=block.insns[: index + 1], end=insn.end, ends=split.Ends.LEAVES, succ=())
                break
        trimmed[block.at] = block
    reached, pending = set(), [entry]
    while pending:
        at = pending.pop()
        if at in reached or at not in trimmed:
            continue
        reached.add(at)
        pending.extend(trimmed[at].succ)
    return [block for at, block in trimmed.items() if at in reached]


def _weighted_cost(blocks, entry, module_, found: Counter, trips: int) -> None:
    blocks = _execution_blocks(blocks, entry, runtime.for_module(module_))
    depth = loopy.depth(blocks, entry)
    formatter = iced_x86.Formatter(iced_x86.FormatterSyntax.NASM)
    for block in blocks:
        weight = trips ** min(depth.get(block.at, 0), 3)
        for one in block.insns:
            if one.at in module_.calls:
                found["cost"] += CALLED.get((module_.calls[one.at] or "").upper(), CALL) * weight
                continue
            name = formatter.format_mnemonic(one.insn).split()[-1].lower()
            memory = tuple(ir.INFO.info(one.insn).used_memory())
            touches = sum((access.access in ir.READS) + (access.access in ir.WRITES) for access in memory)
            if name in FLOAT:
                found["cost"] += (FLOAT[name] + TOUCH * touches) * weight
                found["a floating-point operation"] += 1
                continue
            cycles = CYCLES.get(name, 2)
            # Alias precision is not execution cost. Unknown array addresses
            # and spill slots still access memory after register allocation.
            cycles += TOUCH * touches
            found["cost"] += cycles * weight


def _module_cost(module_, blocks, found: Counter, trips: int) -> None:
    decoded = ir.decode_module(module_)
    if isinstance(decoded, str):
        raise Unmeasured(decoded)
    covered = set()
    for decoded_body in decoded:
        body = decoded_body.body
        mine = [block for block in blocks if any(lo <= block.at < hi for lo, hi in body.ranges)]
        if not mine or body.seed not in {block.at for block in mine}:
            raise Unmeasured(f"no decoded entry for {body.name}")
        starts = {block.at for block in mine}
        if covered & starts:
            raise Unmeasured("decoded bodies overlap")
        covered |= starts
        _weighted_cost(mine, body.seed, module_, found, trips)
    if covered != {block.at for block in blocks}:
        raise Unmeasured("decoded bodies do not cover every code block")


def _registers(body, module_, found: Counter, hints: mir.AllocationHints) -> None:
    """How many variables a loop touches, against how many registers it uses.

    The question the roadmap never asked, and the one that says plainly
    there is no allocation here: BC emits a statement at a time, so a value
    lives in a register only for as long as one statement needs it. Every
    variable is re-read from memory in the next statement even when the loop
    touches six of them and the machine has six registers free.
    """
    inside = loopy.loops(list(body.blocks), body.entry)
    at_of = {block.at: block for block in body.blocks}
    for loop in inside:
        cells: set[str] = set()
        registers: set[str] = set()
        traffic = 0
        for at in loop.body:
            for op in at_of[at].ops:
                if op.at in module_.calls:
                    continue
                for ref in (one for one in (*op.loads, *op.stores) if named(one)):
                    cells.add(str(ref.addr))
                    traffic += 1
                for value in (*op.defines, *op.uses):
                    got = hints.origin_of(value)
                    if got is None:
                        continue
                    name = NAMED.get(got, "").upper().removeprefix("E")
                    if name in GENERAL:
                        registers.add(name)
        if not cells:
            continue
        found["loops seen"] += 1
        found[f"  a loop touching {len(cells)} variables in {len(registers)} registers"] += 1
        if len(cells) <= len(GENERAL):
            found["  every variable in the loop would fit in registers"] += 1
            found["  memory accesses that would become none"] += traffic


def _invariant(body, module_, found: Counter) -> None:
    """Reads inside a loop of a cell the loop never writes.

    The LICM question, and the one a memory-redundancy count cannot ask: BC
    reloads a variable every iteration whether or not the loop touches it.
    `hotlop.bas` is the shape -- `mov ax,[n] / imul word [k]` runs on every
    pass through a loop that writes neither, and both are constants besides.

    Counted per site rather than per iteration, so it says how much code
    could move rather than how much time it costs.
    """
    inside = loopy.loops(list(body.blocks), body.entry)
    if not inside:
        return
    at_of = {block.at: block for block in body.blocks}
    for loop in inside:
        written: set[str] = set()
        opaque = False
        for at in loop.body:
            for op in at_of[at].ops:
                if op.barrier or op.at in module_.calls:
                    opaque = True
                written.update(str(one.addr) for one in op.stores if named(one))
        if opaque:
            continue  # a call in the loop may write anything
        for at in loop.body:
            for op in at_of[at].ops:
                for ref in (one for one in op.loads if named(one)):
                    if str(ref.addr) not in written:
                        found["read inside a loop of a cell the loop never writes"] += 1


class Unmeasured(Exception):
    """The instrument cannot account for the program, not a zero-cost program."""


def counted(paths: list[Path], raw: bool = False) -> Counter:
    """Cross-block, because BC's redundancy is loop-carried.

    Block-scoped was the floor and it is the wrong floor: the loop counter
    round-trips through memory every iteration and the reload arrives over
    the back-edge, which a per-block walk cannot see at all.

    A forward fixed point, intersected at a join -- a cell counts as written
    only where *every* path into the block wrote it and none read it since.
    Starting from nothing and growing is the conservative direction: a cycle
    cannot talk itself into a fact, so this under-counts rather than over-.
    """
    found: Counter = Counter()
    for path in paths:
        module_ = _measured(path, raw)
        if module_ is None:
            raise Unmeasured(f"{path}: no code module")
        if split.event_enabled(module_):
            found["event-enabled configuration"] += 1
        if "B$LINA" in module_.calls.values():
            # A /D build tracks the current line at every statement and checks
            # every subscript. Every target in docs/measurement/targets.md is BC's own plain
            # build, so the two are not the same program -- harr-bounds scored
            # 30,578 against HARR's 1,834 and read as a 16.67x miss.
            found["checked configuration"] += 1
        mapped = code_map(module_)
        if isinstance(mapped, str):
            raise Unmeasured(f"{path}: {mapped}")
        physical = split.partition(module_, mapped)
        _module_cost(module_, physical, found, TRIPS.get(_program(path, module_.records), 10))
        bodies = mir.bodies(module_, physical)
        raised = {block.at for _, body in bodies for block in body.blocks}
        found["code blocks unavailable to MIR opportunity analysis"] += len({block.at for block in physical} - raised)
        for _name, body in bodies:
            blocks = {block.at: block for block in body.blocks}
            preds: dict[int, list[int]] = {at: [] for at in blocks}
            for block in body.blocks:
                for successor in block.succ:
                    if successor in preds:
                        preds[successor].append(block.at)
            exits: dict[int, tuple[dict[mir.MemRef, int], set[mir.MemRef]]] = {at: ({}, set()) for at in blocks}

            for _round in range(len(blocks) + 2):
                changing = False
                for at in sorted(blocks):
                    entering: dict[mir.MemRef, int] | None = None
                    seen: set[mir.MemRef] | None = None
                    for previous in preds[at]:
                        was, had = exits[previous]
                        if entering is None or seen is None:
                            entering, seen = dict(was), set(had)
                        else:
                            entering = {c: v for c, v in entering.items() if c in was}
                            seen &= had
                    got = _through(blocks[at], module_, entering or {}, seen or set(), None)
                    if got != exits[at]:
                        exits[at] = got
                        changing = True
                if not changing:
                    break

            _invariant(body, module_, found)
            _registers(body, module_, found, bodies.hints[body.entry])
            _reloads(body, module_, found)

            for at in sorted(blocks):
                entering, seen = None, None
                for previous in preds[at]:
                    was, had = exits[previous]
                    if entering is None or seen is None:
                        entering, seen = dict(was), set(had)
                    else:
                        entering = {c: v for c, v in entering.items() if c in was}
                        seen &= had
                _through(blocks[at], module_, entering or {}, seen or set(), found)
    return found


# The best case, per program, hard-coded. Every counter's target is zero --
# a compiler that leaves a redundant load or an invariant read in a loop has
# left something on the table by definition. The cost target is the cost of
# the hand-written optimal listing in docs/measurement/targets.md, computed with the
# same formula this file uses, so the two are comparable.
#
# Where docs/measurement/targets.md carries a full optimal listing the number is derived
# from it; the four marked `~` are scaled from the same savings applied to a
# body that has not been written out by hand yet, and are the weakest thing
# here. They are targets, not measurements: the point is to have a number to
# close on rather than to be right about it to the cycle.
TARGETS = {
    # Every one derived by hand from the full listing, both sides: BC's own
    # body costed instruction by instruction with the formula below, and the
    # optimal listing in docs/measurement/targets.md costed the same way. Where the hand
    # total differs from what this file computes -- the header bytes BC puts
    # before the first instruction decode as instructions, and a block's
    # depth is not always what reading the listing suggests -- the target is
    # the hand ratio applied to the measured cost, and both numbers are in
    # docs/measurement/targets.md.
    # Optimal loop body x the same trip count the measurement uses, plus
    # the straight-line part. Weighting one side and not the other was the
    # last mistake in this file: it made matrix read 18x when the honest
    # figure is five.
    "HOTLOP": 312,
    "PRESS": 308,
    "ARRIDX": 660,
    "SUBEXP": 162,
    "IVCHAN": 560,
    "STRIDE": 602,
    "MATRIX": 6210,
    "SPILL": 1122,
    "NESTED": 768,
    "LNGMIX": 210,
    "SPLIT": 276,
    "ADDRM": 754,
    "ROTATE": 294,
    "BOOLS": 116,
    "SEGLD": 6704,
    "HARR": 1834,
    "HG": 304,
    "FPCSE": 157,  # PDS/VBDOS: nine stores, entry checkpoint and complete output.
    "FX": 1038,
    "HOTLPX": 217,  # Complete runtime-input reference in docs/measurement/targets.md.
    # Runtime-input references are derived independently in docs/measurement/targets.md.
    "PRESSX": 508,
    "FPCSEX": 1340,
    "LNGMXX": 208,
    "NOTS": 306,
    "NEGNOT": 254,
    "ARITH": 592,
    "FLAGS": 248,  # Complete constant-branch reference retaining states at every output call.
    "CMPORD": 1918,  # 24 three-call rows, DONE and termination; the initial stores are unobservable.
    "CHAIN": 482,  # Twelve numeric stores, seven label/result rows, DONE and termination.
    "JUMPS": 742,  # Three expanded iterations, twelve stores and six four-call output rows.
    "FPDEEP": 1317,  # PDS: 1086 output + 21 stores + 21 pending-exception checks.
    # Complete source-derived listings are documented in docs/measurement/targets.md.
    "DIVMOD": 1174,
    "FPEMU": 696,
    "PROCS": 533,
}


PROVISIONAL_TARGETS = {
    "FPCSEX": "reference reassociates the sum and omits SINGLE rounding",
    "FPDEEP": "complete reference is verified only for ordinary non-resumable PDS builds",
}


# These objects establish decoding, OMF, debug-type or historical regression
# behavior.  They are correctness fixtures, not members of the cross-compiler
# optimization matrix.  Keeping the classification beside TARGETS prevents a
# new fixture from silently appearing as a missing performance denominator.
NON_BENCHMARK_PROGRAMS = {
    "BYREF2": "CodeView/by-reference ABI fixture; only QB debug builds exist",
    "CM": "inherited compiler-module OMF fixture, not a suite program",
    "JT": "inherited jump-table OMF fixture, not a suite program",
    "NESTUD": "CodeView nested-type fixture; only QB debug builds exist",
    "RCFLIP": "single-toolchain regression fixture, outside the cross-compiler matrix",
    "WENDGO": "single-toolchain CFG regression fixture, outside the cross-compiler matrix",
}


def _scope(program: str, found: Counter) -> str | None:
    """Why a row is outside the release-code optimization comparison.

    Event polling and checked builds deliberately execute work absent from the
    plain reference.  Calling their plain denominator provisional mixed a
    correctness configuration into the optimization gate; excluding it here
    keeps both gates exact.  Strict runtime-input FP remains visible as
    provisional below because it is a release-code optimization question.
    """
    if program in NON_BENCHMARK_PROGRAMS:
        return NON_BENCHMARK_PROGRAMS[program]
    if found.get("event-enabled configuration"):
        return "event instrumentation requires its own correctness run, not a plain-code cost target"
    if found.get("checked configuration"):
        return "checked instrumentation requires its own correctness run, not a plain-code cost target"
    return None


def against_targets(paths: list[Path], raw: bool = False) -> int:
    """Every program against its best case, and how far off we are.

    The counters are the easy half: a perfect compiler leaves none of them,
    so the target is zero and the gap is the count. The cost is the number
    to close, and the ratio is what says whether a pass earned its place.
    """
    failed = False
    print(f"  {'program':10s} {'cost':>7s} {'target':>7s} {'ratio':>6s}   redundancy left")
    for path in sorted(paths):
        try:
            found = counted([path], raw)
        except Unmeasured as why:
            print(f"  {path.stem:10s} UNMEASURED: {why}")
            failed = True
            continue
        cost = found.pop("cost", 0)
        program = _program(path)
        scope = _scope(program, found)
        want = TARGETS.get(program)
        left = sum(count for name, count in found.items() if name.startswith(("load ", "store ", "read ")))
        if scope:
            print(f"  {path.stem:10s} {cost:7d} {'--':>7s} {'--':>6s}   {left}  OUT OF SCOPE: {scope}")
            continue
        if want is None:
            failed = True
            print(f"  {path.stem:10s} {cost:7d} {'--':>7s} {'--':>6s}   {left}  NO TARGET")
            continue
        reason = PROVISIONAL_TARGETS.get(program)
        if program == "FPDEEP" and path.is_file():
            reference_module = module.load(path)
            flags = int.from_bytes(reference_module.code[split.U_FLAG : split.U_FLAG + 2], "little")
            # u_sw_x permits resumable errors; that requires a separate reference.
            if (
                module.family(reference_module.records) is module.Family.PDS
                and split.has_header(reference_module)
                and not flags & (split.EVENTS | 0x20)
            ):
                reason = None
        if program == "CHAIN":
            reference_module = module.load(path)
            if sum(name == "B$PEI4" for name in reference_module.calls.values()) != 7:
                reason = (
                    "CHAIN reference requires the current seven-row source; this object has different output coverage"
                )
        if program == "DIVMOD":
            reference_module = module.load(path)
            numeric = sum(
                name in {"B$PEI2", "B$PEI4"} for name in reference_module.calls.values()
            )
            if numeric != 20:
                reason = "DIVMOD reference requires the current twenty-result source"
        if program == "PROCS":
            reference_module = module.load(path)
            if sum(name == "REPORT" for name in reference_module.calls.values()) != 3:
                reason = "PROCS reference requires all three source calls"
        if program == "FPCSE":
            family = module.family(omf.parse(path.read_bytes())) if path.is_file() else module.Family.UNKNOWN
            if family is module.Family.QUICKBASIC:
                want = 145  # Entry checkpoint precedes initialization: seven final stores.
            elif family not in (module.Family.PDS, module.Family.VBDOS):
                reason = "FPCSE requires an identified compiler for its entry-checkpoint reference"
        if reason:
            failed = True
            print(f"  {path.stem:10s} {cost:7d} {want:7d} {'--':>6s}   {left}  PROVISIONAL: {reason}")
            continue
        ratio = cost / want if want else 0
        failed |= cost * 2 > want * 3
        print(f"  {path.stem:10s} {cost:7d} {want:7d} {ratio:5.2f}x   {left}")
    return int(failed)


def _measured(path: Path, raw: bool):
    """The module to cost: what we ship, or what BC wrote.

    Costing the parsed object was measuring BC and calling it our score --
    every ratio on this board sat still no matter what a pass did, which is
    exactly the reading that should never have been believed. The default
    is now our own output.
    """
    data = path.read_bytes()
    if not raw:
        try:
            data, _regions = rewrite.rewrite(data, dry_run=False)
        except rewrite.Unsupported as error:
            # A strict fresh-emitter refusal is neither BC's code nor an
            # optimized result.  Let the target loop report this module and
            # continue auditing the rest of the corpus.
            raise Unmeasured(f"{path}: fresh emission refused: {error}") from error
        if omf.finalised_at(omf.parse(data)) is None:
            raise Unmeasured(f"{path}: LIR emission did not complete")
    return module.of(omf.parse(data))


def spilling(paths: list[Path], raw: bool = False) -> None:
    """The per-variable ranking an allocator would spill by."""
    for path in paths:
        module_ = _measured(path, raw)
        if module_ is None:
            continue
        mapped = code_map(module_)
        if isinstance(mapped, str):
            continue
        for name, body in mir.bodies(module_, split.partition(module_, mapped)):
            costs = _spill_cost(body, module_)
            if not costs:
                continue
            print(f"  {path.stem} {name}")
            for cell, cost in costs.most_common(12):
                print(f"      {cost:6d}  {cell}")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="opportunity")
    ap.add_argument("objects", nargs="*", type=Path)
    ap.add_argument("--raw", action="store_true", help="cost BC's object instead of our output")
    ap.add_argument("--spill", action="store_true", help="rank each variable by what it costs to keep in memory")
    ap.add_argument("--targets", action="store_true", help="every program against its hard-coded best case")
    args = ap.parse_args(argv)
    paths = args.objects or sorted(Path("fixtures/omf").glob("*.obj"))
    if args.spill:
        spilling(paths, args.raw)
        return 0
    if args.targets:
        return against_targets(paths, args.raw)
    found = counted(paths, args.raw)
    print(f"  {found.pop('cost', 0):8d}  COST -- weighted cycles, the number to minimise")
    for name, count in sorted(found.items(), key=lambda one: -one[1]):
        print(f"  {count:8d}  {name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
