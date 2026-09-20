"""
Every real pipeline stage's own output, written to disk per object --
every stage -- so a failure can be inspected directly
instead of re-instrumented or re-run.

    uv run python tools/dump.py fixtures/omf/nbody-v-g3.obj
    uv run python tools/dump.py fixtures/omf          # every object in a directory

Written under build/dump/<object stem>/, one file per stage:
module, blocks, extent, ir, live, loops, mir, regalloc, analysis.
live.txt is MIR value liveness, one line per operation -- what a disassembly
alone cannot show, and what a manual audit otherwise has to reconstruct.
"""

import sys
import argparse
from pathlib import Path

from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

sys.path.insert(0, str(Path(__file__).resolve().parent))

from qbopt.model import ir
from qbopt.model import mir
from qbopt.frontend import wide
from qbopt.analysis import loops
from qbopt.backend import target
from qbopt.objectfile import omf
from qbopt.analysis import consts
from qbopt.frontend import blocks
from qbopt.frontend import extent
from qbopt.legacy import regalloc
from qbopt.analysis import liveness
from qbopt.objectfile import module

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


def _values(values) -> str:
    return "{" + ",".join(map(str, sorted(values, key=lambda value: value.id))) + "}"


def dump_live(found: module.Module, found_blocks: list[blocks.Block] | None) -> str:
    """Per-operation MIR value liveness for every successfully raised body."""
    if found_blocks is None:
        return "code_map failed -- no blocks to analyse\n"
    out: list[str] = []
    for name, body in _bodies_in_mir(found, found_blocks):
        live = liveness.live(body)
        out.append(name)
        for block in body.blocks:
            after = {}
            alive = set(live.live_out[block.at])
            for operation in reversed(block.ops):
                after[id(operation)] = frozenset(alive)
                alive -= set(operation.defines)
                alive |= set(operation.uses)
            out.append(f"  {block.at:#06x} in={_values(live.live_in[block.at])} out={_values(live.live_out[block.at])}")
            for operation in block.ops:
                incoming = (set(after[id(operation)]) - set(operation.defines)) | set(operation.uses)
                out.append(
                    f"    {operation.at:#06x} in={_values(incoming)} "
                    f"out={_values(after[id(operation)])} {operation.kind.value}"
                )
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
        case ir.St(index=index):
            return f"st({index})"


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


def dump_loops(found_blocks: list[blocks.Block] | None) -> str:
    if not found_blocks:
        return "code_map failed -- no blocks\n"
    idom = loops.immediate_dominators(found_blocks)
    frontier = loops.frontiers(found_blocks)
    depth = loops.depth(found_blocks)
    out = [f"irreducible: {sorted(hex(one) for one in loops.irreducible(found_blocks)) or 'none'}", "", "loops:"]
    for loop in loops.loops(found_blocks):
        out.append(
            f"  header {loop.header:#06x}  latches {sorted(hex(x) for x in loop.latches)}  {len(loop.body)} blocks"
        )
    out += ["", "blocks:"]
    for block in found_blocks:
        parent = idom.get(block.at)
        out.append(
            f"  {block.at:#06x}  idom={parent if parent is None else hex(parent)}"
            f"  depth={depth.get(block.at, 0)}"
            f"  DF={sorted(hex(x) for x in frontier.get(block.at, ()))}"
        )
    return "\n".join(out)


def _bodies_in_mir(found: module.Module, found_blocks: list[blocks.Block]) -> list[tuple[str, mir.MirBody]]:
    result = ir.decode_module(found)
    if isinstance(result, str):
        return []
    nodes = {ir.span(n)[0]: n for body in result for n in body.nodes}
    out = []
    for body in result:
        mine = [b for b in found_blocks if any(lo <= b.at < hi for lo, hi in body.body.ranges)]
        if not mine:
            continue
        built = mir.raise_body(mine, nodes, body.body.seed, found.calls)
        if not isinstance(built, str):
            out.append((f"{body.body.kind} {body.body.name or '(main)'}", built))
    return out


def dump_mir(found: module.Module, found_blocks: list[blocks.Block] | None) -> str:
    if not found_blocks:
        return "code_map failed -- no blocks\n"
    out: list[str] = []
    for label, body in _bodies_in_mir(found, found_blocks):
        out.append(f"{label}  entry {body.entry:#06x}  {len(body.values)} values")
        for block in body.blocks:
            out.append(f"  {block.at:#06x}  succ={[hex(s) for s in block.succ]}")
            for phi in block.phis:
                where = {hex(k): str(v) for k, v in phi.incoming.items()}
                out.append(f"      phi {phi.result} <- {where}")
            for op in block.ops:
                mem = ""
                if op.loads:
                    mem += "  ld " + ",".join(str(c.addr) for c in op.loads)
                if op.stores:
                    mem += "  st " + ",".join(str(c.addr) for c in op.stores)
                out.append(f"      {op.at:#06x} {op.name:<7} {list(op.defines)} <- {list(op.uses)}{mem}")
        out.append("")
    return "\n".join(out)


def dump_regalloc(found: module.Module, found_blocks: list[blocks.Block] | None) -> str:
    if not found_blocks:
        return "code_map failed -- no blocks\n"
    out: list[str] = []
    for label, body in _bodies_in_mir(found, found_blocks):
        alive = liveness.live(body)
        assignment = regalloc.colour(body)
        peak = regalloc.pressure(body, alive)
        out.append(f"{label}  peak pressure {peak}/{len(target.AVAILABLE)}")
        if isinstance(assignment, str):
            out.append(f"  refused: {assignment}")
        else:
            out.append(f"  moved from BC's own register: {regalloc.moved(body, assignment)}")
        arriving = liveness.entry_values(body)
        out.append(f"  values the caller supplied: {sorted(str(v) for v in arriving)}")
        for block in body.blocks:
            out.append(f"  {block.at:#06x}  in={sorted(str(v) for v in alive.live_in[block.at])}")
            out.append(f"          out={sorted(str(v) for v in alive.live_out[block.at])}")
        out.append("")
    return "\n".join(out)


def dump_analysis(found: module.Module, found_blocks: list[blocks.Block] | None) -> str:
    """What the transforms would act on, per body."""
    if not found_blocks:
        return "code_map failed -- no blocks\n"
    out: list[str] = []
    for label, body in _bodies_in_mir(found, found_blocks):
        out += [f"{label}:"]
        facts = consts.known(body)
        out.append(f"  known constants: {len(facts)}")
        for value, fact in sorted(facts.items(), key=lambda kv: kv[0].id)[:40]:
            out.append(f"      {value} = {fact}")
        pairs = wide.pairs(body)
        out.append(f"  32-bit pairs BC wrote as two: {len(pairs)}")
        for pair in pairs:
            out.append(f"      {pair.low.at:#06x}+{pair.high.at:#06x}  {pair.low.name}+{pair.high.name} -> {pair.op}")
        tests = wide.tests(body, found.calls)
        out.append(f"  comparison-and-branch: {len(tests)}")
        for one in tests[:40]:
            through = f" through {one.through}" if one.through else ""
            out.append(f"      {one.compare.at:#06x} -> {one.branch.at:#06x}  {one.test}{through}")
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
    (out / "live.txt").write_text(dump_live(found, found_blocks) + "\n")
    (out / "loops.txt").write_text(dump_loops(found_blocks) + "\n")
    (out / "mir.txt").write_text(dump_mir(found, found_blocks) + "\n")
    (out / "regalloc.txt").write_text(dump_regalloc(found, found_blocks) + "\n")
    (out / "analysis.txt").write_text(dump_analysis(found, found_blocks) + "\n")
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
