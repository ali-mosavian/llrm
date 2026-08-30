"""
Every real pipeline stage's own output, written to disk per object --
module, blocks, extent, ir -- so a failure can be inspected directly
instead of re-instrumented or re-run.

    uv run python tools/dump.py fixtures/omf/nbody-v-g3.obj
    uv run python tools/dump.py fixtures/omf          # every object in a directory

Written under build/dump/<object stem>/{module,blocks,extent,ir}.txt.
"""

import sys
import argparse
from pathlib import Path

from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

sys.path.insert(0, str(Path(__file__).resolve().parent))

from qbopt import ir
from qbopt import omf
from qbopt import blocks
from qbopt import extent
from qbopt import module

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "build" / "dump"
_FMT = Formatter(FormatterSyntax.NASM)


def dump_module(found: module.Module) -> str:
    return "\n".join(
        [
            f"seg={found.seg} name={found.name!r} code={len(found.code)} start={found.start:#06x} end={found.end:#06x}",
            f"dgroup={sorted(found.dgroup)}",
            f"operands={len(found.operands)} calls={len(found.calls)} targets={len(found.targets)}"
            f" publics={len(found.publics)}",
            f"lines={len(found.lines)} sites={len(found.sites)} chunks={list(found.chunks)}",
            "",
            "calls:",
            *(f"  {at:#06x}  {name}" for at, name in sorted(found.calls.items())),
        ]
    )


def dump_blocks(mapped: blocks.CodeMap | str, found: list[blocks.Block] | None) -> str:
    if isinstance(mapped, str):
        return f"code_map failed: {mapped}"
    return "\n".join(
        [
            f"starts={len(mapped.starts)} leaders={len(mapped.leaders)}",
            f"tables={list(mapped.tables)}",
            f"unreached={list(mapped.unreached)}",
            "",
            "blocks:",
            *(f"  {b.at:#06x}-{b.end:#06x}  {b.ends}  succ={list(b.succ)}" for b in found or ()),
        ]
    )


def dump_extent(found: extent.Partition | str) -> str:
    if isinstance(found, str):
        return f"partition failed: {found}"
    return "\n".join(
        [
            f"complete={found.complete}",
            f"benign={list(found.benign)}",
            f"unexplained={list(found.unexplained)}",
            f"conflicts={list(found.conflicts)}",
            "",
            "bodies:",
            *(f"  {b.kind:<10} {b.name or '':<20} {list(b.ranges)}  ({b.length} bytes)" for b in found.bodies),
        ]
    )


def _regs(regs: frozenset | None) -> str:
    if regs is None:
        return "ANY"
    return "{" + ",".join(sorted(_FMT.format_register(r) for r in regs)) + "}"


def _node_line(node: ir.Node) -> str:
    at, end = ir.span(node)
    match node:
        case ir.Opaque(insn=insn, effects=effects):
            return (
                f"  {at:#06x}-{end:#06x}  Opaque   {insn.insn}"
                f"  defs={_regs(effects.defs)} uses={_regs(effects.uses)} flags={effects.flags_written.name}"
            )
        case ir.Long(insn=insn, decoded=decoded):
            return f"  {at:#06x}-{end:#06x}  Long     {decoded.kind:<8} {insn.insn}"
        case ir.Call(insn=insn, name=name):
            return f"  {at:#06x}-{end:#06x}  Call     {name}  {insn.insn}"
        case ir.Restore(pair=pair):
            return f"  {at:#06x}-{end:#06x}  Restore  pair={pair}"
        case ir.Data(kind=kind, entries=entries):
            return f"  {at:#06x}-{end:#06x}  Data     {kind}  {len(entries)} entries {list(entries)}"


def dump_ir(result: tuple[ir.BodyIR, ...] | str) -> str:
    if isinstance(result, str):
        return f"decode_module failed: {result}"
    out: list[str] = []
    for body_ir in result:
        b = body_ir.body
        out.append(f"{b.kind} {b.name or ''} {list(b.ranges)}  {len(body_ir.nodes)} nodes")
        out += [_node_line(n) for n in body_ir.nodes]
        out.append("")
    return "\n".join(out)


def dump_one(path: Path) -> Path:
    found = module.of(omf.parse(path.read_bytes()))
    out = OUT / path.stem
    out.mkdir(parents=True, exist_ok=True)
    if found is None:
        (out / "module.txt").write_text("module.of() found no code segment\n")
        return out

    (out / "module.txt").write_text(dump_module(found) + "\n")

    mapped = blocks.code_map(found)
    found_blocks = blocks.partition(found, mapped) if not isinstance(mapped, str) else None
    (out / "blocks.txt").write_text(dump_blocks(mapped, found_blocks) + "\n")

    found_extent = extent.partition(found)
    (out / "extent.txt").write_text(dump_extent(found_extent) + "\n")

    (out / "ir.txt").write_text(dump_ir(ir.decode_module(found)) + "\n")
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="dump")
    ap.add_argument("where", type=Path, nargs="+")
    args = ap.parse_args(argv)

    paths = [p for w in args.where for p in ([w] if w.is_file() else sorted({*w.glob("*.obj"), *w.glob("*.OBJ")}))]
    for path in paths:
        out = dump_one(path)
        print(f"{path.name} -> {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
