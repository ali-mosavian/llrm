"""Every pass's effect on one object, stage by stage.

Two views, because the bugs live between them. The MIR view is what a pass
decided; the object view is what came back after the bytes were written and
re-parsed. A pass whose MIR still has the loop but whose re-parsed object
does not has lost a branch in emission, which is the shape of every
placement bug so far -- and the shape nothing in the host suite can see.

    uv run python tools/stages.py fixtures/omf/hotlop-p-g2.obj
    uv run python tools/stages.py fixtures/omf/hotlop-p-g2.obj --only hoist
    uv run python tools/stages.py fixtures/omf/hotlop-p-g2.obj --asm
"""

import sys
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from iced_x86 import Decoder
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

from qbopt import mir
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
    args = ap.parse_args(argv)

    data = args.object.read_bytes()
    was = _report("BC", data, None, not args.quiet)
    if args.asm:
        _asm(data)

    # Emission with no pass at all, which is the control: anything that
    # changes here is the emitter and not a transform.
    plain, why = rebuilt(data, optimise=False)
    was = _report(f"emitted, no pass ({why})", plain, was, not args.quiet)
    if args.asm:
        _asm(plain)

    for name in [args.only] if args.only else PASSES:
        out, why = rebuilt(data if args.only else plain, only=name)
        was = _report(f"{name} ({why})", out, was, not args.quiet)
        if args.asm:
            _asm(out)
        if not args.only:
            plain = out
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
