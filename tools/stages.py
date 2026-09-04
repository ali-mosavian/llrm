"""Every pass's effect on one object, stage by stage.

Two views, because the bugs live between them. The MIR view is what a pass
decided; the object view is what came back after the bytes were written and
re-parsed. A pass whose MIR still has the loop but whose re-parsed object
does not has lost a branch in emission, which is the shape of every
placement bug so far -- and the shape nothing in the host suite can see.

    uv run python tools/stages.py fixtures/omf/hotlop-p-g2.obj
    uv run python tools/stages.py fixtures/omf/hotlop-p-g2.obj --only hoist
    uv run python tools/stages.py fixtures/omf/hotlop-p-g2.obj --asm

The MIR absorb pass runs here by default, which is *not* what rewrite.py
does: it passes `absorb=not absorb_calls`, so with the default settings
calls.py absorbs in the machine arm and the MIR pass is switched off. That
default is right for the pipeline and wrong for this tool -- it would list
an `absorb` stage that cannot fire, and report no change on a program with
fourteen absorbable calls. `--no-absorb` gives the pipeline's own setting.
"""

import sys
import argparse
import contextlib
import itertools
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from iced_x86 import Decoder
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

from qbopt import ir
from qbopt import mir
from qbopt import lower
from qbopt import regalloc
from iced_x86 import Register

_REGISTERS = {v: k for k, v in vars(Register).items() if isinstance(v, int)}
from qbopt import omf
from qbopt import module
from qbopt import loops as loopy
from qbopt import blocks as split
from qbopt.blocks import code_map
from qbopt.declen import BITNESS
from qbopt.transform import PASSES
from qbopt.wholeseg import rebuilt


def _bodies(data: bytes):
    """Every MIR body in this object, or nothing if it does not map."""
    found = module.of(omf.parse(data))
    if found is None:
        return None, []
    mapped = code_map(found)
    if isinstance(mapped, str):
        return found, []
    return found, list(mir.bodies(found, split.partition(found, mapped)))


def _shape(body) -> str:
    """Blocks, successors and op counts on one line, with the loop count."""
    found = loopy.loops(list(body.blocks), body.entry)
    drawn = " ".join(
        f"{one.at:#x}->{','.join(f'{s:#x}' for s in one.succ) or '-'}[{len(one.ops)}]" for one in body.blocks
    )
    return f"{len(found)} loop(s) | {drawn}"


def _ops(body) -> dict[int, list[tuple[int, str]]]:
    return {one.at: [(op.at, op.name or "?") for op in one.ops] for one in body.blocks}


def _report(tag: str, data: bytes, was: dict | None, verbose: bool) -> dict:
    """One stage: what the object holds now, against what it held before."""
    found, bodies = _bodies(data)
    print(f"\n=== {tag}  ({len(data)} bytes, code {len(found.code) if found else 0})")
    now = {}
    for name, body in bodies:
        print(f"  {name}: {_shape(body)}")
        now[name] = _ops(body)
        if was is None or not verbose:
            continue
        before = was.get(name, {})
        gone = {at for ops in before.values() for at, _n in ops} - {at for ops in now[name].values() for at, _n in ops}
        for at, ops in now[name].items():
            came = [
                f"{op:#x} {n}"
                for op, n in ops
                if not any(op == b for b, _x in before.get(at, ()))
            ]
            if came:
                print(f"      into {at:#x}: " + ", ".join(came))
        if gone:
            print("      dropped: " + ", ".join(f"{one:#x}" for one in sorted(gone)))
    return now


def _short(one) -> str:
    """One MIR operand, short enough to diff."""
    if isinstance(one, mir.Held):
        return f"{one.value}:{one.width}"
    if isinstance(one, mir.Const):
        return f"#{one.n}:{one.width}"
    if isinstance(one, mir.Cell):
        return _cell(one.ref)
    return f"opaque({getattr(one, 'what', one)})"


def _cell(ref) -> str:
    through = f"+{ref.base}" if getattr(ref, "base", None) is not None else ""
    return f"{ref.addr}{through}:{ref.width}"


def _mir(bodies) -> None:
    """What each pass decided, in MIR's own terms and nothing else.

    Rule 4 asks for the MIR between passes, not the code at the end. No
    register appears here: an operand is a value, a constant or a cell, and
    that is the whole vocabulary.
    """
    print("  --- mir")
    for name, body in bodies:
        print(f"  {name}")
        for block in body.blocks:
            succ = ",".join(f"{one:#x}" for one in block.succ) or "-"
            print(f"    block {block.at:#06x} -> {succ}")
            for phi in block.phis:
                came = " ".join(f"{at:#x}:{value}" for at, value in sorted(phi.incoming.items()))
                print(f"      phi {phi.result} <- {came}")
            for op in block.ops:
                parts = [f"{op.at:#06x}", f"{op.name or '?':8s}"]
                if op.defines:
                    parts.append("=> " + ",".join(str(one) for one in op.defines))
                if op.uses:
                    parts.append("<= " + ",".join(str(one) for one in op.uses))
                if op.args:
                    parts.append("args " + ",".join(_short(one) for one in op.args))
                if op.results:
                    parts.append("into " + ",".join(_short(one) for one in op.results))
                if op.loads:
                    parts.append("ld " + ",".join(_cell(one) for one in op.loads))
                if op.stores:
                    parts.append("st " + ",".join(_cell(one) for one in op.stores))
                if op.target is not None:
                    parts.append(f"-> {op.target:#x}")
                if op.raised is not None and (op.args, op.results) != op.raised:
                    parts.append("(rewritten)")
                print("      " + "  ".join(parts))


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
    print("  --- code")
    for insn in Decoder(BITNESS, bytes(found.code), ip=0):
        print(f"    {insn.ip:#06x}  {formatter.format(insn)}")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="stages")
    ap.add_argument("object", type=Path)
    ap.add_argument("--only", help="one pass by name, instead of each in turn")
    ap.add_argument("--asm", action="store_true", help="disassemble after every stage")
    ap.add_argument("--quiet", action="store_true", help="shape only, no per-op detail")
    ap.add_argument(
        "--dump",
        type=Path,
        metavar="DIR",
        help="three files per stage -- s<N>-{mir,lir,asm}-<stage>.txt -- so "
        "`diff` between two adjacent ones is the whole answer; rule 4 asks "
        "for the mir pair",
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

    @contextlib.contextmanager
    def view(number: int, form: str, name: str):
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

    def dump(number: int, name: str, tag: str, out: bytes, was, verbose: bool):
        """Every view of one stage: what it decided, what that lowers to,
        and what came back after the bytes were written and re-parsed."""
        _, bodies = _bodies(out)
        with view(number, "mir", name):
            now = _report(tag, out, was, verbose)
            _mir(bodies)
        with view(number, "lir", name):
            print(f"=== {tag}")
            _lir(bodies)
        if args.dump is not None or args.asm:
            with view(number, "asm", name):
                print(f"=== {tag}")
                _asm(out)
        return now

    was = dump(next(step), "omf", "BC", data, None, not args.quiet)

    # Emission with no pass at all, which is the control: anything that
    # changes here is the emitter and not a transform.
    plain, why = rebuilt(data, optimise=False)
    was = dump(next(step), "emitted", f"emitted, no pass ({why})", plain, was, not args.quiet)

    # Widening is the one step that is not a pass: it recognises an idiom
    # and writes machine form, so wholeseg runs it after every pass and
    # before lowering. Rule 4 asks for every step, and on a program whose
    # arithmetic is all longs it is the only one that fires -- leaving it
    # out of the list left the dump saying nothing happened.
    for name in [args.only] if args.only else (*PASSES, "widen"):
        out, why = rebuilt(data if args.only else plain, only=name)
        was = dump(next(step), name, f"{name} ({why})", out, was, not args.quiet)
        if not args.only:
            plain = out
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
