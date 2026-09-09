"""
Every pass's effect on one object, as MIR.

Rule 4: diff the MIR between passes, not the emitted code at the end. One
file per stage, `s<N>-mir-<stage>.txt`, so `diff s06-mir-hoist.txt
s07-mir-forward.txt` is the whole answer.

    uv run python tools/stages.py fixtures/omf/hotlop-p-g2.obj
    uv run python tools/stages.py fixtures/omf/hotlop-p-g2.obj --only hoist
    uv run python tools/stages.py fixtures/omf/hotlop-p-g2.obj --dump /tmp/st

**The machine views are dumped once, at the end.** A pass between the raise
and lowering has no machine form -- that is the architecture -- and an lir
and an asm file beside every stage said the opposite.

It used to lower and re-raise after every pass, and claimed that was the
only way to get the SSA back. It is not: `mir.resolved()` puts a body in SSA
again after a pass has moved things, which is what a pass itself uses when it
asks a question about its own result. So the passes run here the way they run
in wholeseg -- all of them on one body, lowering once at the end -- and each
stage costs a pass rather than a pass plus a machine round trip.

Widening is in the list and is not a pass. It recognises an idiom -- a long
written as two halves joined by a carry -- and writes machine form, so
wholeseg runs it after every pass and before lowering. On a program whose
arithmetic is all longs it is the only step that fires, and leaving it out
made the dump say nothing had happened.
"""

import sys
import argparse
import itertools
import contextlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from iced_x86 import Register
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.objectfile import cvinfo
from qbopt.legacy import regalloc
from qbopt.frontend import fpstack

_REGISTERS = {v: k for k, v in vars(Register).items() if isinstance(v, int)}
from qbopt.objectfile import omf
from qbopt.objectfile import module
from qbopt import wholeseg
from qbopt.analysis import loops as loopy
from qbopt.frontend import blocks as split
from qbopt.frontend.blocks import code_map


def _bodies(data: bytes):
    """Every MIR body in this object, or nothing if it does not map."""
    found = module.of(omf.parse(data))
    if found is None:
        return None, [], {}
    mapped = code_map(found)
    if isinstance(mapped, str):
        return found, [], {}
    from qbopt.abi import runtime

    # One map for the whole run, kept on the module the tool passes on, so
    # the raise and the lowering are given the same object -- built twice
    # they can differ, which is what wholeseg.py takes care not to do.
    contracts = runtime.for_module(found)
    return found, list(mir.bodies(found, split.partition(found, mapped), contracts)), contracts


def _shape(body) -> str:
    """Blocks, successors and op counts on one line, with the loop count."""
    found = loopy.loops(list(body.blocks), body.entry)
    drawn = " ".join(
        f"{one.at:#x}->{','.join(f'{s:#x}' for s in one.succ) or '-'}[{len(one.ops)}]" for one in body.blocks
    )
    return f"{len(found)} loop(s) | {drawn}"


def _ops(body) -> dict[int, list[tuple[int, str]]]:
    return {one.at: [(op.at, op.name or "?") for op in one.ops] for one in body.blocks}


def _report(tag: str, bodies, was: dict | None, verbose: bool) -> dict:
    """One stage: the shape of each body now, against what it was."""
    print(f"\n=== {tag}")
    now = {}
    for name, body in bodies:
        print(f"  {name}: {_shape(body)}")
        now[name] = _ops(body)
        if was is None or not verbose:
            continue
        before = was.get(name, {})
        gone = {at for ops in before.values() for at, _n in ops} - {at for ops in now[name].values() for at, _n in ops}
        for at, ops in now[name].items():
            came = [f"{op:#x} {n}" for op, n in ops if not any(op == b for b, _x in before.get(at, ()))]
            if came:
                print(f"      into {at:#x}: " + ", ".join(came))
        if gone:
            print("      dropped: " + ", ".join(f"{one:#x}" for one in sorted(gone)))
    return now


# Operators, so a line reads as arithmetic rather than as a list of
# opcodes. Only where there is one: a float add stays `fadd` because `+`
# would say it is the same operation, and it is not.
_SYMBOL = {
    mir.Kind.ADD: "+",
    mir.Kind.SUB: "-",
    mir.Kind.MUL: "*",
    mir.Kind.DIV: "/",
    mir.Kind.REM: "%",
    mir.Kind.AND: "&",
    mir.Kind.OR: "|",
    mir.Kind.XOR: "^",
    mir.Kind.SHL: "<<",
    mir.Kind.SHR: ">>",
    mir.Kind.SAR: ">>>",
}
_PREFIX = {mir.Kind.NEG: "-", mir.Kind.NOT: "~", mir.Kind.ADDRESS: "&"}


# How wide each of BASIC's own types is, so an access narrower than the
# variable can be named as the half it is rather than as an offset.
_SIZE = {"INTEGER": 2, "LONG": 4, "SINGLE": 4, "DOUBLE": 8, "STRING": 4}


def _part(into: int, size: int, width: int) -> str:
    """What is being read out of a variable, where it is not the whole of it.

    A four-byte variable read two bytes at a time is BC's whole output --
    every long is written as two halves -- and `DELTAX&` against
    `DELTAX&+2` said two variables where there is one read twice. MIR
    carries the width on every operand; this is where it is finally shown.
    """
    if not size or width >= size:
        return ""
    if width * 2 == size:
        return ".lo" if into == 0 else ".hi" if into == width else f"+{into}"
    return f"+{into}:{width}" if into else f":{width}"


class Cells:
    """What each cell one body touches is called, and the legend for them.

    With /Zi the object says: BC writes every variable's name, type and
    address into $$SYMBOLS, and every procedure's parameters and locals
    with their own bp offsets. Then a cell is `POSX&` rather than a letter,
    and which slot is a parameter is a fact rather than an inference.

    Without it -- an object built before /Zi was required -- the fallback
    below is the letter and the legend.

    `[seg:5+si+0x6]` is a segment index and a displacement, and nbody has
    twenty of them: unreadable in a statement and identical to each other at
    a glance. A letter at first use and a table at the end says the same
    thing once.

    Frame slots keep their own naming -- `L14` for a local fourteen bytes
    below bp, `P6` for a parameter six above it -- because which one a slot
    is, is the useful fact about it, and BC's frame layout says so.
    """

    def __init__(self, debug=None) -> None:
        self.named: dict = {}
        self.order: list = []
        # (segment, offset) -> what BC calls it. Segment indices in a fixup
        # are 1-based against SEGDEF and cvinfo's are the same, so the two
        # can be compared directly.
        # Sorted, and looked up by the nearest one at or below the address:
        # a LONG at 0x76 is read as two halves at 0x76 and 0x78, and an
        # array element is a displacement from the array's own base. Naming
        # only the exact address left every high half and every element
        # anonymous.
        # An array is registered where its elements are, not where its
        # descriptor is: BC files the descriptor in BC_CN and the elements
        # in BC_DATA, and the code only ever names the elements.
        self.symbols = sorted(
            (one.data or (one.segment, one.offset), one.name, one.stride or _SIZE.get(one.type_name or "", 0))
            for one in (debug.variables if debug is not None else ())
        )
        self.slots = {
            one.bp_offset: (one.name, one.type_name or "")
            for proc in (debug.procedures if debug is not None else ())
            for one in proc.locals
        }

    def of(self, ref) -> str:
        if ref.addr is None:
            return "[?]"
        # The value the address is reached through is the index, and it is
        # part of the statement rather than of the cell: `E[v121]` and
        # `E[v128]` are two elements of one array, and keying the name on
        # the index would have made them two arrays.
        index = f"[{ref.base}]" if getattr(ref, "base", None) is not None else ""
        space = getattr(ref.addr.space, "name", "")
        if space == "FRAME":
            disp = ref.addr.disp
            said = self.slots.get(disp)
            if said is not None:
                return said[0] + index
            return (f"L{-disp:x}" if disp < 0 else f"P{disp:x}") + index
        if space == "STACK":
            return f"push{ref.addr.disp:+d}" + index
        said = self._inside(ref.addr.index, ref.addr.disp)
        if said is not None:
            name, into, size = said
            # The index comes before what is read out of the element: the
            # high half of `POSX&(i)` is `POSX&[v121].hi`.
            return f"{name}{index}{_part(into, size, ref.width)}"
        key = (str(ref.addr), ref.width)
        if key not in self.named:
            number = len(self.order)
            # A, B ... Z, then AA. Twenty-six is more than any body here.
            name = chr(ord("A") + number % 26) * (1 + number // 26)
            self.named[key] = name
            self.order.append((name, ref.addr, ref.width))
        return f"{self.named[key]}{index}:{ref.width}"

    def _inside(self, segment: int, disp: int):
        """(name, how far into it, how wide the variable is), or None."""
        import bisect

        at = bisect.bisect_right(self.symbols, ((segment, disp), "\xff", 0)) - 1
        if at < 0:
            return None
        (where, start), name, size = self.symbols[at]
        if where != segment or disp < start or disp - start > 0x100:
            return None
        return name, disp - start, size

    def legend(self) -> list[str]:
        return [f"      {name:4s} {addr} :{width}" for name, addr, width in self.order]


def _short(one, cells: Cells) -> str:
    """One MIR operand."""
    if isinstance(one, mir.Held):
        return f"{one.value}"
    if isinstance(one, mir.Const):
        return f"{one.n}"
    if isinstance(one, mir.Symbol):
        return f"&{one.space}:{one.index}+{one.offset + one.addend:#x}"
    if isinstance(one, mir.Cell):
        return cells.of(one.ref)
    return one.name or "?"


def _mir(bodies, found=None, verbose: bool = False, debug=None) -> None:
    """What each pass decided, as `c := a op b` and nothing else.

    Rule 4 asks for the MIR between passes, not the code at the end. No
    register appears here: an operand is a value, a constant or a cell, and
    that is the whole vocabulary.
    """
    print("  --- mir")
    calls = (found.calls if found is not None else {}) or {}
    # By name, not by address: the object's symbols keep BC's own offsets
    # and every pass moves the code, so after the first stage the addresses
    # no longer meet. BASIC's type suffix is not part of the label.
    procs = {one.name.rstrip("&%!#$"): one for one in (debug.procedures if debug is not None else ())}
    for name, body in bodies:
        print(f"  {name}")
        for line in _signature(procs.get(name.split()[-1])):
            print(line)
        cells = Cells(debug)
        depth = _depth(body)
        floats = fpstack.readings(body)
        from qbopt.analysis import floatfacts
        exact = floatfacts.known(body, found.dgroup, calls) if found is not None else {}
        for block in body.blocks:
            pad = "  " * depth.get(block.at, 0)
            succ = ", ".join(f"{one:#x}" for one in block.succ) or "-"
            print(f"\n    {block.at:#06x}  {pad}-> {succ}")
            for phi in block.phis:
                came = ", ".join(f"{at:#x}:{value}" for at, value in sorted(phi.incoming.items()))
                print(f"    {'':6s}  {pad}{phi.result} := phi {came}")
            for op in block.ops:
                # The address stays in a gutter: it is what `diff` between
                # two stages keys on, and rule 4 is why these files exist.
                flow = floats.get(op.at) if op.stack is not None and op.floating_origin is None else None
                values = ""
                if flow is not None and (flow.uses or flow.defines is not None):
                    uses = ", ".join(str(value) for value in flow.uses.values()) or "-"
                    values = f"  ; fp values: {uses} -> {flow.defines or '-'}"
                if op.floating is not None:
                    rule = op.floating
                    inputs = ",".join(rule.inputs)
                    values += (f"  ; {inputs} -> {rule.result}; precision={rule.precision}"
                               f" rounding={rule.rounding} exceptions={rule.exceptions}")
                    numeric = [exact[arg.value] for arg in op.results if isinstance(arg, mir.Held) and arg.value in exact]
                    if numeric:
                        values += " exact=" + ",".join("-0" if fact.negative_zero else str(fact.value) for fact in numeric)
                print(f"    {op.at:#06x}  {pad}{_says(op, cells, calls, verbose)}{values}")
        if cells.order:
            print("\n    where:")
            for line in cells.legend():
                print(line)


def _signature(proc) -> list[str]:
    """What a procedure takes and keeps, where the object says so.

    Which slot is a parameter and which a local is the sign of its own bp
    offset -- the caller pushed the one above the frame pointer -- and BC
    writes both, so nothing here has to read the prologue to find out.
    """
    if proc is None:
        return []
    params = [one for one in proc.locals if one.bp_offset > 0]
    keeps = [one for one in proc.locals if one.bp_offset < 0]
    out = [f"      {proc.name} ({', '.join(_declared(one) for one in params) or '-'})"]
    if keeps:
        out.append(f"      locals  {', '.join(_declared(one) for one in keeps)}")
    return out


def _declared(one) -> str:
    return f"{one.name} {one.type_name or '?'} at bp{one.bp_offset:+d}"


def _depth(body) -> dict:
    """How deeply nested each block is, so the structure is visible."""
    from qbopt.analysis import loops as loopy

    out: dict = {}
    for loop in loopy.loops(list(body.blocks), body.entry):
        for at in loop.body:
            out[at] = out.get(at, 0) + 1
    return out


def _says(op, cells: Cells, calls: dict, verbose: bool) -> str:
    """One operation, in three-address form."""
    args = [_short(one, cells) for one in op.args]
    into = ", ".join(_short(one, cells) for one in op.results)

    notes = []
    for ref in dict.fromkeys(op.loads + op.stores):
        if ref.allocation is not None:
            notes.append(f"in bounds {_short(mir.Cell(ref), cells)} of {_short(ref.allocation, cells)}")
    if op.memory_values:
        notes.append(
            "on return "
            + ", ".join(f"{_short(mir.Cell(ref), cells)}={_short(value, cells)}" for ref, value in op.memory_values)
        )
    if op.array is not None:
        request = op.array
        bounds = ", ".join(f"{low}..{high}" for low, high in request.bounds)
        action = "replace array" if request.replaces else "allocate array"
        notes.append(
            f"request {action} {_short(request.descriptor, cells)} ({bounds}), element {request.element_width}"
        )
    if verbose:
        flags = [one for one in op.uses if one.flags]
        if flags:
            notes.append("with " + ", ".join(str(one) for one in flags))
        if op.merges:
            notes.append("keeps " + ", ".join(str(one) for one in op.merges))
    said = ("    ; " + "; ".join(notes)) if notes else ""

    if op.kind is mir.Kind.JUMP:
        return f"goto {op.target:#x}" if op.target is not None else "goto ?"
    if op.kind is mir.Kind.BRANCH:
        asked = op.test.name.lower() if op.test is not None else "?"
        where = f" goto {op.target:#x}" if op.target is not None else ""
        reads = ", ".join(str(one) for one in op.uses) or "?"
        return f"if {reads} {asked}{where}{said}"
    if op.kind is mir.Kind.CALL:
        made = ", ".join(str(one) for one in op.defines if not one.flags)
        who = calls.get(op.at, "")
        return f"{made + ' := ' if made else ''}call {who}".rstrip() + said
    if op.kind is mir.Kind.ARG:
        return f"arg {args[0] if args else '?'}{said}"
    if op.kind in (mir.Kind.COPY, mir.Kind.LOAD, mir.Kind.STORE) and into and len(args) == 1:
        return f"{into} := {args[0]}{said}"
    if op.kind in _SYMBOL and len(args) == 2:
        return f"{into + ' := ' if into else ''}{args[0]} {_SYMBOL[op.kind]} {args[1]}{said}"
    if op.kind in _PREFIX and len(args) == 1:
        return f"{into + ' := ' if into else ''}{_PREFIX[op.kind]}{args[0]}{said}"
    kind = op.kind.name.lower()
    if not into:
        return f"{kind} {', '.join(args)}".rstrip() + said
    return f"{into} := {kind} {', '.join(args)}".rstrip() + said


def _machine(stages, view, number: int) -> int:
    """Each machine phase of the run that produced the object, one file each.

    Rule 4 asks for a file per stage so that `diff` between two adjacent
    ones is the whole answer. Recomputing the phases here to get them
    dumped a *different* program -- a lowering handed the absorbed set
    instead of the record, no coverage, no pins and one shared frame -- so
    the allocation in the dump was not the one that wrote the bytes, and
    the first bad transition was not in it. These come from the emitter's
    own run, through the watch wholeseg.emitted takes.
    """
    for stage, bodies in stages:
        with view(number, "lir", stage):
            print(f"=== {stage}")
            for name, one in bodies:
                _lir_body(name, one)
        number += 1
    return number


def _lir_body(name: str, body) -> None:
    """One lowered body: its instructions, and what each operand is now."""
    print(f"  {name}: {len(body.insns)} instructions")
    for block in body.blocks:
        print(f"    block {block.at:#06x}")
        for phi in block.phis:
            arms = " ".join(f"{at:#06x}:v{value}" for at, value in phi.incoming)
            print(f"      v{phi.result} := phi {arms}")
        for one in block.insns:
            what = one.what
            if what is None:
                print(f"      {one.at:#06x}  (carried, {one.covers})")
                continue
            dests = ", ".join(_operand(x) for x in what.dests)
            sources = ", ".join(_operand(x) for x in what.sources)
            said = f"{dests} := " if dests else ""
            print(f"      {one.at:#06x}  {said}{what.name} {sources}".rstrip())


def _operand(one) -> str:
    """One machine operand, short enough to diff."""
    from qbopt.model import ir

    if isinstance(one, ir.Reg):
        return _name_of(one.register)
    if isinstance(one, ir.Held):
        return f"v{one.value}"
    if isinstance(one, ir.Imm):
        return f"{one.value:#x}" if one.value >= 0 else str(one.value)
    if isinstance(one, ir.Mem):
        if one.base is None:
            return f"[{one.addr}]"
        # A cell that names the value which computed its address, and the
        # register that value was placed in. Both, because the dump is what
        # rule 4 diffs: printing the address alone made a cell whose base
        # was never placed look exactly like one BC addressed itself.
        placed = _name_of(one.through) if one.through != Register.NONE else "unplaced"
        return f"[{one.addr} v{one.base.value}@{placed}]"
    return str(one)


def _lir(bodies) -> None:
    """The same operations after lowering, and where the allocator put them.

    This is the first place a register is allowed to appear. An operation
    nothing rewrote lowers to None and is emitted from its own bytes.
    """
    print("  --- lir")
    for name, body in bodies:
        fixed = regalloc.untangled(body)
        got = regalloc.colour(fixed, fixed.pins)
        if isinstance(got, str):
            print(f"  {name}: the allocator refuses -- {got}")
            got, fixed = {}, body
        else:
            print(f"  {name}: {len(got)} values seated")
            seats = [f"{value}={_name_of(where)}" for value, where in sorted(got.items(), key=lambda kv: kv[0].id)]
            # Eight to a line so a diff points at the values that moved
            # rather than at one line three hundred entries wide.
            for at in range(0, len(seats), 8):
                print("    " + " ".join(seats[at : at + 8]))
        for block in fixed.blocks:
            print(f"    block {block.at:#06x}")
            for op in block.ops:
                what = lower.current(op)
                if what is None:
                    print(f"      {op.at:#06x}  {op.name or '?':8s} verbatim")
                    continue
                dests = ",".join(_loc(one) for one in what.dests) or "-"
                sources = ",".join(_loc(one) for one in what.sources) or "-"
                target = f"  -> {what.target:#x}" if what.target is not None else ""
                print(f"      {op.at:#06x}  {what.name or '?':8s} {dests} <- {sources}{target}")


def _name_of(where) -> str:
    return _REGISTERS.get(where, str(where))


def _loc(one) -> str:
    """One machine operand."""
    if isinstance(one, ir.Reg):
        return f"{_name_of(one.register)}:{one.width}"
    if isinstance(one, ir.Held):
        return f"held({one.value}):{one.width}"
    if isinstance(one, ir.Imm):
        return f"#{one.value}:{one.width}"
    if isinstance(one, ir.Mem):
        return f"{one.addr}:{one.width}"
    return type(one).__name__


def _asm(data: bytes) -> None:
    found = module.of(omf.parse(data))
    if found is None:
        return
    formatter = Formatter(FormatterSyntax.NASM)
    print("  --- reachable code (FP emulator instructions shown as x87 equivalents)")
    instructions = split.instructions(found)
    if isinstance(instructions, str):
        print(f"  --- cannot map code: {instructions}")
        return
    for insn in instructions:
        print(f"    {insn.at:#06x}  {formatter.format(insn.insn)}")


def main(argv: list[str] | None = None, view=None) -> int:
    ap = argparse.ArgumentParser(prog="stages")
    ap.add_argument("object", type=Path)
    ap.add_argument("--only", help="one pass by name, instead of each in turn")
    ap.add_argument("--asm", action="store_true", help="disassemble what came out, after the last stage")
    ap.add_argument("--quiet", action="store_true", help="shape only, no per-op detail")
    ap.add_argument(
        "--verbose",
        action="store_true",
        help="show what an operation reads that is not an operand -- the flags "
        "value the machine handed it and the halves it only preserves. Both are "
        "the machine artifacts MIR still carries, so they are off by default and "
        "worth turning on when one of them is the question",
    )
    ap.add_argument(
        "--dump",
        type=Path,
        metavar="DIR",
        help="one file per stage -- s<N>-mir-<stage>.txt -- so `diff` between "
        "two adjacent ones is the whole answer, which is what rule 4 asks for. "
        "The machine views are dumped once at the end, where lowering happens: "
        "a pass between the raise and lowering has no machine form",
    )
    ap.add_argument(
        "--no-absorb",
        dest="absorb",
        action="store_false",
        help="leave absorption to the machine arm, which is what rewrite.py does by default",
    )
    args = ap.parse_args(argv)

    data = args.object.read_bytes()
    if args.dump is not None:
        args.dump.mkdir(parents=True, exist_ok=True)

    step = itertools.count()
    given = view

    @contextlib.contextmanager
    def writing(number: int, form: str, name: str):
        """One form of one stage, in its own file where --dump asks for one.

        `s<N>-<form>-<name>.txt`. Numbered in the order they run and
        zero-padded, because the point of having them is
        `diff s06-mir-hoist.txt s07-mir-forward.txt` -- sorting by name has
        to put them in pipeline order for that to be one keystroke, and
        `s10` sorts before `s2`. The form comes before the name so the three
        views of one stage sit together and every mir view of the pipeline
        reads in order.
        """
        if args.dump is None:
            yield
            return
        path = args.dump / f"s{number:02d}-{form}-{name}.txt"
        with path.open("w") as handle, contextlib.redirect_stdout(handle):
            yield
        print(f"  {path}")

    # A caller of its own, for a test that wants the text rather than a
    # directory of files.
    view = given or writing

    def dump(number: int, name: str, tag: str, bodies, was, debug, found):
        """One stage, as MIR. There is no other form of one."""
        with view(number, "mir", name):
            now = _report(tag, bodies, was, not args.quiet)
            _mir(bodies, found, args.verbose, debug)
        return now

    def lowered(number: int, stages, out: bytes, why: str, route: str):
        """The machine views, once: this is where lowering happens.

        The bodies the passes above produced, not a second raise of the
        emitted bytes. Raising the output again lowers a different
        program -- s20 beside s13 then compares two -- and a first-pass
        miscompile reads there as an extra phi in the second pass.
        """
        if args.dump is None and not args.asm:
            return
        number = _machine(stages, view, number)
        with view(number, "asm", "emitted"):
            print(f"=== emitted ({why}, {len(out)} bytes)")
            print(f"  --- {route}")
            _asm(out)

    found, raised, contracts = _bodies(data)
    debug = cvinfo.parse(omf.parse(data))
    if found is None or not raised:
        print("  nothing to raise")
        return 1

    was = dump(next(step), "omf", f"BC ({len(data)} bytes)", raised, None, debug, found)

    # Observe the actual optimization run; never rebuild or re-resolve it for a dump.
    mir_stages: dict[str, list] = {}
    stages: list[tuple[str, list]] = []
    route = "the route was not reported"

    def watch(stage: str, name, low) -> None:
        nonlocal route
        if stage.startswith("mir-"):
            mir_stages.setdefault(stage.removeprefix("mir-"), []).append((name, low))
            return
        if stage == "route":
            route = low
            return
        if not stages or stages[-1][0] != stage:
            stages.append((stage, []))
        stages[-1][1].append((name, low))

    got = wholeseg.emitted(data, only=args.only, watch=watch)
    for name, bodies in mir_stages.items():
        was = dump(next(step), name, name, bodies, was, debug, found)
    lowered(next(step), stages, got.data, got.reason, route)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
