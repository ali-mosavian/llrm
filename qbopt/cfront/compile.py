"""C through Open Watcom's front end and qbopt's backend, to jwasm source.

    python -m qbopt.cfront pal.c -o pal.asm -I src [--dump DIR] [--opt]

The front end is owshim/bin/wccq (owshim/build.sh). A `.cgs` stream it
already wrote is accepted in place of the source.
"""

import os
import argparse
import tempfile
import subprocess
from pathlib import Path

from qbopt import flow
from qbopt.model import mir
from qbopt.cfront import hir
from qbopt.backend import masm
from qbopt.backend import lower
from qbopt.cfront import stream
from qbopt.backend import prologue
from qbopt.cfront import raise_hir
from qbopt.backend import frame as frames

WCCQ = Path(__file__).resolve().parents[2] / "owshim" / "bin" / "wccq"
# Borland's medium model: far code, near data, cdecl, byte-packed structs,
# 16-bit enums, x87 inline, no stack probes, no default library. -fp3 is for
# inline assembly: qcport's own uses 387 instructions.
FLAGS = (
    "-mm", "-3", "-fpi87", "-fp3", "-zp1", "-ei", "-ecc", "-s", "-zl", "-zq",
    f"-fi={Path(__file__).with_name('borland.h')}",
)  # fmt: skip


def recorded(source: Path, includes: list[str]) -> str:
    """The code-generator stream wccq records for one C file."""
    with tempfile.TemporaryDirectory() as scratch:
        out = Path(scratch) / "unit.cgs"
        searched = tuple(f"-I{Path(one).resolve()}" for one in includes)
        command = [str(WCCQ), *FLAGS, *searched, f"-fo={scratch}/unit.obj", str(source.resolve())]
        # In the scratch directory, where wccq also leaves its .err file.
        environment = {**os.environ, "QBOPT_CG_STREAM": str(out)}
        done = subprocess.run(command, env=environment, capture_output=True, text=True, cwd=scratch)
        if done.returncode != 0 or not out.exists():
            raise hir.Unsupported(f"wccq failed on {source}:\n{done.stdout}{done.stderr}")
        return out.read_text()


def compiled(text: str, module: str, *, optimise: bool = False, dump: Path | None = None) -> str:
    unit = hir.unit(stream.parse(text))
    _write(dump, "stream", text)
    _write(dump, "hir", hir.text(unit))
    procedures, mirs, lirs = [], [], []
    shared = raise_hir.Shared()
    for proc in unit.procs:
        raised = raise_hir.raised(unit, proc, shared)
        body = raised.body
        mirs.append(_mir_text(raised.name, body))
        if optimise:
            from qbopt.optimize import transform

            def watch(stage: str, after: mir.MirBody, name: str = raised.name) -> None:
                _write(dump, f"passes/{name}.{stage}", _mir_text(name, after))

            body = transform.applied(body, frozenset(), raised.calls, found=None, watch=watch if dump else None)
            mirs.append(_mir_text(raised.name + " (opt)", body))
        low = lower.lowered(raised.name, body, raised.calls, {}, raised.contracts, {}, "386")
        lirs.append(_lir_text(raised.name, low))
        frame = frames.of(low, raised.calls)
        for number, phase in enumerate(flow.machine(flow._pinned(low), frame, raised.calls)):
            if not isinstance(phase, prologue.Prologue):
                low = phase.transform(low)
                _write(dump, f"phases/{raised.name}.{number:02d}-{type(phase).__name__}", _lir_text(raised.name, low))
        lirs.append(_lir_text(raised.name + " (allocated)", low))
        reserve = -min(min(frame.slots.values(), default=0), frame.floor)
        callees = {
            at: masm.Callee(one.object_name, one.far, raised.inline.get(at, ())) for at, one in raised.callees.items()
        }
        procedures.append(masm.Procedure(raised.name, raised.symbol.exported, raised.symbol.far, low, reserve, callees))
    _write(dump, "mir", "\n".join(mirs))
    _write(dump, "lir", "\n".join(lirs))
    text = masm.text(
        masm.Module(
            code=f"{module.upper()}_TEXT",
            names=raise_hir.names(unit, shared),
            externs=_externs(unit) + tuple((one.object_name, "far") for one in shared.runtime.values()),
            publics=tuple(one.object_name for one in unit.symbols.values() if one.exported),
            data=(*_data(unit), *_literals(shared)),
            procedures=tuple(procedures),
            private=frozenset(one.name for one in unit.segments.values() if one.attr & hir.PRIVATE),
        )
    )
    _write(dump, "asm", text)
    return text


def _externs(unit: hir.Unit) -> tuple[tuple[str, str], ...]:
    return tuple(
        (one.object_name, ("far" if one.far else "near") if one.proc else "byte")
        for one in unit.symbols.values()
        if one.imported and one.code is None and one.name not in raise_hir.EMITTED
    )


def _data(unit: hir.Unit):
    """Each data segment's items as jwasm lines."""
    widths = {1: "db", 2: "dw", 4: "dd"}
    for segment in unit.segments.values():
        if not segment.items or segment.attr & 0x1:  # EXEC: code has no data items
            continue
        lines = []
        for call, args in segment.items:
            match call, args:
                case "DGLabel", (back,):
                    symbol = unit.backs[hir.handle(back)]
                    label = unit.symbols[symbol].object_name if symbol else f"L_b{hir.handle(back)}"
                    lines.append(f"{label} label byte")
                case "DGUBytes", (size,):
                    lines.append(f"    db {size} dup (?)" if segment.name == "_BSS" else f"    db {size} dup (0)")
                case "DGIBytes", (size, byte):
                    lines.append(f"    db {size} dup ({byte})")
                case "DGBytes", (size, data):
                    lines += [
                        "    db " + ",".join(f"0{data[i:i + 2]}h" for i in range(start, min(len(data), start + 32), 2))
                        for start in range(0, len(data), 32)
                    ]
                case "DGInteger", (value, type_):
                    # The shim prints a negative item as its 32-bit two's complement.
                    width = raise_hir.WIDTHS.get(type_, 2)
                    lines.append(f"    {widths[width]} {int(value) & ((1 << (8 * width)) - 1)}")
                case "DGFEPtr", (symbol, type_, offset):
                    name = unit.symbols[hir.handle(symbol)].object_name
                    far = type_ in raise_hir.FAR_POINTERS or type_ in ("TY_LONG_CODE_PTR", "TY_CODE_PTR")
                    lines.append(f"    {'dd' if far else 'dw'} {name}{'+' + offset if offset != '0' else ''}")
                case "DGBackPtr", (back, _segment, offset, type_):
                    symbol = unit.backs[hir.handle(back)]
                    name = unit.symbols[symbol].object_name if symbol else f"L_b{hir.handle(back)}"
                    far = type_ in raise_hir.FAR_POINTERS
                    lines.append(f"    {'dd' if far else 'dw'} {name}{'+' + offset if offset != '0' else ''}")
                case "DGAlign", (align,):
                    lines.append(f"    align {align}")
                case _:
                    raise hir.Unsupported(f"data item {call} {' '.join(args)}")
        yield segment.name, tuple(lines)


def _literals(shared: raise_hir.Shared):
    """The float constants the raise placed, in DGROUP's constant segment."""
    lines = []
    for packed, number in shared.literals.items():
        lines += [f"L_f{number} label byte", "    db " + ",".join(f"0{byte:02x}h" for byte in packed)]
    return (("CONST", tuple(lines)),) if lines else ()


def _mir_text(name: str, body: mir.MirBody) -> str:
    out = [f"== {name}"]
    for block in body.blocks:
        out.append(f"block {block.at} -> {block.succ}")
        out += [f"  phi {phi}" for phi in block.phis]
        for op in block.ops:
            extra = f" test={op.test} target={op.target}" if op.test or op.target is not None else ""
            out.append(f"  {op.at:4} {op.kind} {op.args} -> {op.results}{extra}")
    return "\n".join(out) + "\n"


def _lir_text(name: str, body) -> str:
    out = [f"== {name}"]
    for block in body.blocks:
        out.append(f"block {block.at} -> {block.succ}")
        out += [f"  phi {phi}" for phi in block.phis]
        out += [f"  {one.at:4} {one.what} req={one.requires} del={one.delivers}" for one in block.insns]
    return "\n".join(out) + "\n"


def _write(dump: Path | None, stage: str, text: str) -> None:
    if dump is not None:
        (dump / stage).parent.mkdir(parents=True, exist_ok=True)
        (dump / stage).write_text(text)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m qbopt.cfront", description=__doc__.splitlines()[0])
    parser.add_argument("source", type=Path)
    parser.add_argument("-o", "--output", type=Path)
    parser.add_argument("-I", "--include", action="append", default=[])
    parser.add_argument("--dump", type=Path)
    parser.add_argument("--opt", action="store_true")
    args = parser.parse_args(argv)
    text = args.source.read_text() if args.source.suffix == ".cgs" else recorded(args.source, args.include)
    output = args.output or args.source.with_suffix(".asm")
    output.write_text(compiled(text, args.source.stem, optimise=args.opt, dump=args.dump))
    return 0
