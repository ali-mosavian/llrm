"""C through Open Watcom's front end and qbopt's backend, to an object or jwasm source.

    python -m qbopt.cfront pal.c -o pal.obj -I src [--dump DIR] [--opt]

An `.obj` output is written here; anything else is jwasm source.

The front end is owshim/bin/wccq (owshim/build.sh). A `.cgs` stream it
already wrote is accepted in place of the source.
"""

import os
import argparse
import tempfile
import subprocess
from pathlib import Path
from collections.abc import Callable
from collections.abc import Iterator

from qbopt import flow
from qbopt.model import lir
from qbopt.model import mir
from qbopt.cfront import hir
from qbopt.backend import masm
from qbopt.backend import jumps
from qbopt.backend import lower
from qbopt.cfront import stream
from qbopt.cfront import libfunc
from qbopt.backend import phielim
from qbopt.backend import omfwrite
from qbopt.backend import prologue
from qbopt.cfront import raise_hir
from qbopt.backend import lower_int64
from qbopt.backend import cpu as targets
from qbopt.backend import frame as frames

WCCQ = Path(__file__).resolve().parents[2] / "owshim" / "bin" / "wccq"
# Borland's medium model: far code, near data, cdecl, byte-packed structs,
# 16-bit enums, x87 inline, no stack probes, no default library. -fp3 is for
# inline assembly: qcport's own uses 387 instructions.
FLAGS = (
    "-mm", "-3", "-fpi87", "-fp3", "-zp1", "-ei", "-ecc", "-s", "-zl", "-zq",
    f"-fi={Path(__file__).with_name('borland.h')}",
)  # fmt: skip

type Watch = Callable[[str, str, object], None]


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


def assembled(
    text: str,
    module: str,
    *,
    optimise: bool = False,
    dump: Path | None = None,
    cpu: str | targets.Profile = "386",
    watch: Watch | None = None,
) -> masm.Module:
    target = targets.profile(cpu)
    unit = hir.unit(stream.parse(text))
    _write(dump, "stream", text)
    _write(dump, "hir", hir.text(unit))
    procedures, mirs, lirs = [], [], []
    shared = raise_hir.Shared()
    raised_procedures = [raise_hir.raised(unit, proc, shared) for proc in unit.procs]
    from qbopt.analysis import alias

    aliases = {one.name: alias.Procedure(one.body, one.calls, one.arguments) for one in raised_procedures}
    callees = {name for one in raised_procedures for name in one.calls.values()}
    modref = alias.summaries(aliases, libfunc.summaries(callees))
    for raised in raised_procedures:
        body = alias.calls_annotated(aliases[raised.name], modref)
        if watch is not None:
            watch("mir-raised", raised.name, body)
        mirs.append(_mir_text(raised.name, body))
        if optimise:
            from qbopt.optimize import rotate
            from qbopt.optimize import transform

            def observe(stage: str, after: mir.MirBody, name: str = raised.name) -> None:
                _write(dump, f"passes/{name}.{stage}", _mir_text(name, after))
                if watch is not None:
                    watch(f"mir-{stage}", name, after)

            body = transform.applied(
                body,
                frozenset(),
                raised.calls,
                found=None,
                # Borland's medium-model C ABI preserves SI and DI from the
                # six value registers. A recurrence live through a call has
                # two places available, not the full register file.
                registers=target.register_capacity,
                call_registers=target.call_register_capacity,
                watch=observe if dump is not None or watch is not None else None,
            )
            body = rotate.entered(body)
            if dump is not None or watch is not None:
                observe("rotate", body)
            mirs.append(_mir_text(raised.name + " (opt)", body))
        legalized = lower_int64.expanded(body, raised.calls, raised.contracts)
        body = legalized.body
        if dump and body is not raised.body:
            _write(dump, f"passes/{raised.name}.int64-lower", _mir_text(raised.name, body))
        low = flow.verified(
            lower.lowered(raised.name, body, legalized.calls, {}, legalized.contracts, {}, cpu=target),
            "lower",
            in_ssa=True,
        )
        if watch is not None:
            watch("lir-lower", raised.name, low)
        lirs.append(_lir_text(raised.name, low))
        frame = frames.of(low, legalized.calls)
        in_ssa = True
        for number, phase in enumerate(flow.machine(flow._pinned(low), frame, legalized.calls)):
            if not isinstance(phase, prologue.Prologue):
                if isinstance(phase, phielim.PhiElimination):
                    in_ssa = False
                low = flow.checked(low, phase, in_ssa=in_ssa)
                _write(dump, f"phases/{raised.name}.{number:02d}-{type(phase).__name__}", _lir_text(raised.name, low))
                if watch is not None:
                    watch(f"lir-{phase.name or type(phase).__name__}", raised.name, low)
        low = jumps.threaded(jumps.placed(low))
        if watch is not None:
            watch("lir-layout", raised.name, low)
        lirs.append(_lir_text(raised.name + " (allocated)", low))
        reserve = -min(min(frame.slots.values(), default=0), frame.floor)
        callees = {
            at: masm.Callee(one.object_name, one.far, raised.inline.get(at, ())) for at, one in raised.callees.items()
        }
        callees.update({at: masm.Callee(legalized.calls[at], False, code) for at, code in legalized.inline.items()})
        procedures.append(masm.Procedure(raised.name, raised.symbol.exported, raised.symbol.far, low, reserve, callees))
    _write(dump, "mir", "\n".join(mirs))
    _write(dump, "lir", "\n".join(lirs))
    built = masm.Module(
        code=f"{module.upper()}_TEXT",
        names=raise_hir.names(unit, shared),
        externs=_externs(unit) + tuple((one.object_name, "far") for one in shared.runtime.values()),
        publics=tuple(one.object_name for one in unit.symbols.values() if one.exported),
        data=(*_data(unit), *_literals(shared)),
        procedures=tuple(procedures),
        private=frozenset(one.name for one in unit.segments.values() if one.attr & hir.PRIVATE),
    )
    _write(dump, "asm", masm.text(built))
    return built


def compiled(
    text: str,
    module: str,
    *,
    optimise: bool = False,
    dump: Path | None = None,
    cpu: str | targets.Profile = "386",
    watch: Watch | None = None,
) -> str:
    """The module as jwasm source."""
    return masm.text(assembled(text, module, optimise=optimise, dump=dump, cpu=cpu, watch=watch))


def _externs(unit: hir.Unit) -> tuple[tuple[str, str], ...]:
    return tuple(
        (
            one.object_name,
            ("far" if one.far else "near") if one.proc else "byte" if unit.grouped(one) else "far-byte",
        )
        for one in unit.symbols.values()
        if one.imported and one.code is None and one.name not in raise_hir.EMITTED
    )


def _data(unit: hir.Unit) -> Iterator[tuple[str, tuple[masm.Datum, ...]]]:
    """Each data segment's items."""
    for segment in unit.segments.values():
        if not segment.items or segment.attr & 0x1:  # EXEC: code has no data items
            continue
        items = []
        for call, args in segment.items:
            match call, args:
                case "DGLabel", (back,):
                    symbol = unit.backs[hir.handle(back)]
                    items.append(masm.Label(unit.symbols[symbol].object_name if symbol else f"L_b{hir.handle(back)}"))
                case "DGUBytes", (size,):
                    items.append(masm.Fill(int(size), None if segment.name == "_BSS" else 0))
                case "DGIBytes", (size, byte):
                    items.append(masm.Fill(int(size), int(byte)))
                case "DGBytes", (_size, data):
                    items.append(bytes.fromhex(data))
                case "DGInteger", (value, type_):
                    # The shim prints a negative item as its 32-bit two's complement.
                    width = raise_hir.WIDTHS.get(type_, 2)
                    items.append((int(value) & ((1 << (8 * width)) - 1)).to_bytes(width, "little"))
                case "DGFEPtr", (symbol, type_, offset):
                    far = type_ in raise_hir.FAR_POINTERS or type_ in ("TY_LONG_CODE_PTR", "TY_CODE_PTR")
                    items.append(masm.Pointer(unit.symbols[hir.handle(symbol)].object_name, int(offset), far))
                case "DGBackPtr", (back, _segment, offset, type_):
                    symbol = unit.backs[hir.handle(back)]
                    name = unit.symbols[symbol].object_name if symbol else f"L_b{hir.handle(back)}"
                    items.append(masm.Pointer(name, int(offset), type_ in raise_hir.FAR_POINTERS))
                case "DGAlign", (align,):
                    items.append(masm.Align(int(align)))
                case _:
                    raise hir.Unsupported(f"data item {call} {' '.join(args)}")
        yield segment.name, tuple(items)


def _literals(shared: raise_hir.Shared) -> tuple[tuple[str, tuple[masm.Datum, ...]], ...]:
    """The float constants the raise placed, in DGROUP's constant segment."""
    lines = []
    for packed, number in shared.literals.items():
        lines += [masm.Label(f"L_f{number}"), bytes(packed)]
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


def _lir_text(name: str, body: lir.LirBody) -> str:
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
    parser.add_argument("--cpu", choices=targets.names(), default="386", help="code-generation tuning target")
    args = parser.parse_args(argv)
    text = args.source.read_text() if args.source.suffix == ".cgs" else recorded(args.source, args.include)
    output = args.output or args.source.with_suffix(".asm")
    built = assembled(text, args.source.stem, optimise=args.opt, dump=args.dump, cpu=args.cpu)
    if output.suffix.lower() == ".obj":
        output.write_bytes(omfwrite.written(built, args.source.name))
    else:
        output.write_text(masm.text(built))
    return 0
