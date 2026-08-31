"""
Every real pipeline stage's own output, written to disk per object --
module, blocks, extent, ir, live -- so a failure can be inspected directly
instead of re-instrumented or re-run.

    uv run python tools/dump.py fixtures/omf/nbody-v-g3.obj
    uv run python tools/dump.py fixtures/omf          # every object in a directory

Written under build/dump/<object stem>/{module,blocks,extent,ir,live}.txt.
live.txt is qbopt.registers' own ax/dx/cx/bx liveness, one line per
instruction -- what a disassembly alone can't show, and what a manual audit
of this pass has had to reconstruct by hand more than once.
"""

import sys
import argparse
from pathlib import Path

from iced_x86 import Register
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

sys.path.insert(0, str(Path(__file__).resolve().parent))

from qbopt import ir
from qbopt import omf
from qbopt import blocks
from qbopt import extent
from qbopt import module
from qbopt import registers as regs

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


# ax/dx/cx/bx, in the order registers.Liveness carries them.
_LIVE_TARGETS = (("ax", Register.AX), ("dx", Register.DX), ("cx", Register.CX), ("bx", Register.BX))


def _live_set(block: blocks.Block, at: int, live: regs.Liveness) -> str:
    names = [name for name, target in _LIVE_TARGETS if regs.live_after(block, at, target, getattr(live, name))]
    return "{" + ",".join(names) + "}" if names else "{}"


def dump_live(found_blocks: list[blocks.Block] | None) -> str:
    """Per-instruction ax/dx/cx/bx liveness -- what's still wanted before and
    after each instruction, the thing a plain disassembly can't show and a
    human auditing one by hand has to reconstruct from scratch every time."""
    if found_blocks is None:
        return "code_map failed -- no blocks to analyse\n"
    live = regs.analyse(found_blocks)
    out: list[str] = []
    for block in found_blocks:
        out.append(f"{block.at:#06x}-{block.end:#06x}  {block.ends}  succ={list(block.succ)}")
        for insn in block.insns:
            before = _live_set(block, insn.at, live)
            after = _live_set(block, insn.end, live)
            out.append(f"  {insn.at:#06x}  in={before:<14} out={after:<14} {insn.insn}")
        out.append("")
    return "\n".join(out)


def _loc(where: ir.Loc) -> str:
    match where:
        case ir.Reg(register=register):
            return _FMT.format_register(register)
        case ir.Mem(addr=addr, width=width):
            return f"{width}:{addr if addr is not None else '?'}"
        case ir.Imm(value=value):
            return f"{value:#x}" if value >= 0 else f"-{-value:#x}"
        case ir.Address(addr=addr):
            return f"&{addr if addr is not None else '?'}"


def _semantics(found: ir.Semantics) -> str:
    if not ir.modelled(found):
        return "barrier"
    dests = ",".join(map(_loc, found.dests))
    sources = ",".join(map(_loc, found.sources))
    target = f" -> {found.target:#06x}" if found.target is not None else ""
    return f"{found.name or found.op}({sources})" + (f" -> {dests}" if dests else target)


def _memory(effects: ir.Effects) -> str:
    cells = [f"ld {_loc(one)}" for one in effects.loads] + [f"st {_loc(one)}" for one in effects.stores]
    return " ".join(cells)


def _what(node: ir.Node) -> str:
    match node:
        case ir.Opaque(insn=insn):
            return str(insn.insn)
        case ir.Long(insn=insn, decoded=decoded):
            return f"{decoded.kind:<4} {insn.insn}"
        case ir.Call(insn=insn, name=name):
            return f"{name}  {insn.insn}"
        case ir.Restore(pair=pair):
            return f"pair={pair}"
        case ir.Data(kind=kind, entries=entries):
            return f"{kind}  {len(entries)} entries {list(entries)}"


def _node_line(node: ir.Node) -> str:
    at, end = ir.span(node)
    effects = node.effects
    return (
        f"  {at:#06x}-{end:#06x}  {type(node).__name__:<8} {_what(node):<44}"
        f"  {_semantics(node.semantics):<40}"
        f"  defs={_regs(effects.defs)} uses={_regs(effects.uses)}"
        f" flags={effects.flags_written.name}/{effects.flags_read.name} {_memory(effects)}"
    )


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
    (out / "live.txt").write_text(dump_live(found_blocks) + "\n")
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
