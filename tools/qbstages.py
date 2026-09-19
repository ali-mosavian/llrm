"""Dump every implemented QB source-frontend stage to adjacent text files."""

import argparse
from pathlib import Path
from dataclasses import replace

from qbopt import hir
from qbopt import flow
from qbopt.model import ir
from qbopt.backend import masm
from qbopt.backend import frame
from qbopt.backend import lower
from qbopt.backend import phielim
from qbopt.backend import prologue
from qbopt.frontend.qb import parsed
from qbopt.frontend.qb import finalized
from qbopt.frontend.qb import stage_text
from qbopt.frontend.qb import physicalize
from qbopt.frontend.qb import compile as qb_compile


def _lir(body, callees=None) -> str:
    callees = {} if callees is None else callees
    lines = [f"; entry L0_{body.entry}", f"{body.name} proc"]
    for block in body.blocks:
        successors = ", ".join(f"L0_{one}" for one in block.succ) or "return"
        lines.append(f"L0_{block.at}: ; successors: {successors}")
        for phi in block.phis:
            incoming = ", ".join(f"L0_{at}:v{value}" for at, value in phi.incoming)
            lines.append(f"    ; phi v{phi.result} <- {incoming}")
        for one in block.insns:
            callee = callees.get(one.at)
            if callee is not None and callee.code and one.what is not None and one.what.op is ir.Operation.CALL:
                lines.append(f"    ; inline {callee.name} at {one.at}")
                lines.extend(f"    {line}" for line in stage_text.inline_text(callee.code))
                continue
            if one.what is None:
                lines.append(f"    db ? ; {one.at}: source bytes carried unchanged")
                continue
            rendered = stage_text.instruction_text(one.what)
            lines.extend(f"    {line} ; {one.at}" for line in rendered if line)
    lines.append(f"{body.name} endp")
    return "\n".join(lines) + "\n"


def _emitted_asm(program: hir.Program) -> str:
    """Render the exact return cleanup carried by the emitted assembly model.

    The shared MASM diagnostic printer historically spells every far return as
    bare ``retf`` even when its semantics carry the immediate which the OMF
    writer correctly encodes. Keep this source-frontend showcase truthful
    without changing the shared backend.
    """
    module = qb_compile.assembled(program)
    cleanup: dict[str, int] = {}
    for number, procedure in enumerate(module.procedures):
        for item in masm.listing(procedure, number):
            if (
                isinstance(item, ir.Semantics)
                and item.op is ir.Operation.RETURN
                and item.sources
                and isinstance(item.sources[0], ir.Imm)
            ):
                cleanup[procedure.name] = item.sources[0].value

    current = None
    lines = []
    for line in masm.text(module).splitlines():
        if line.endswith(" proc far") or line.endswith(" proc near"):
            current = line.split(" proc ", 1)[0]
        elif line.endswith(" endp"):
            current = None
        if current in cleanup and line.strip() == "retf":
            line = f"    retf {cleanup[current]}"
        lines.append(line)
    return "\n".join(lines) + "\n"


def dumped(
    source: Path,
    output: Path,
    *,
    dialect: str,
    runtime: str,
    array_order: str = "column-major",
    huge_arrays: bool = False,
    checked_arrays: bool = False,
    mbf: bool = False,
    alternate_math: bool = False,
    includes: tuple[Path, ...],
) -> Path:
    output.mkdir(parents=True, exist_ok=True)
    program = parsed(
        source,
        dialect=dialect,
        runtime=runtime,
        array_order=array_order,
        huge_arrays=huge_arrays,
        checked_arrays=checked_arrays,
        mbf=mbf,
        alternate_math=alternate_math,
        include_dirs=includes,
    )
    semantic = hir.lower(program)
    functions = tuple(function for module in program.modules for function in module.functions)
    (output / "00-input.bas").write_text(source.read_text())
    (output / "01-hir.json").write_text(hir.encode(program))
    for number, (function, body) in enumerate(zip(functions, semantic, strict=True), 1):
        stem = f"{number:02}-{function.name}"
        (output / f"{stem}-02-mir.txt").write_text(hir.mir_text(body))
        body = qb_compile.optimized(program, function, body)
        (output / f"{stem}-03-optimized-mir.txt").write_text(hir.mir_text(body))
        physical = physicalize(program, function, body)
        (output / f"{stem}-04-physical-mir.txt").write_text(hir.mir_text(physical.lowered))
        physical = replace(physical, lowered=qb_compile.optimized_physical(program, function, physical.lowered))
        (output / f"{stem}-05-optimized-physical-mir.txt").write_text(hir.mir_text(physical.lowered))
        ordinary_entry = physical.lowered.body.entry
        ordinary_block = physical.lowered.body.block(ordinary_entry)
        ordinary_fallback = (
            ordinary_block.succ[0] if ordinary_block is not None and len(ordinary_block.succ) == 1 else None
        )
        handler_at = qb_compile._handler_at(function)
        external_entries = tuple(
            dict.fromkeys(
                (
                    *function.external_entries,
                    *(() if handler_at is None else (handler_at,)),
                )
            )
        )
        machine_body, temporary_root = qb_compile._machine_side_entry(
            physical.lowered.body,
            external_entries,
        )
        machine = lower.lowered(
            body.name,
            machine_body,
            physical.calls,
            set(),
            physical.contracts,
            cpu=qb_compile.lowering_target(),
            occurrences={},
            hints=physical.hints,
            pointer_model=physical.pointer_model,
        )
        temporary_blocks = (
            frozenset(block.at for block in machine.blocks)
            - frozenset(block.at for block in physical.lowered.body.blocks)
            if temporary_root is not None
            else frozenset()
        )
        (output / f"{stem}-06-lir.txt").write_text(_lir(machine))
        owned_frame = frame.of(machine, physical.calls, family=program.runtime.value)
        allocated = machine
        in_ssa = True
        stage = 7
        for phase in flow.machine({}, owned_frame, physical.calls, basic_semantics=True):
            if isinstance(phase, prologue.Prologue):
                continue
            if isinstance(phase, phielim.PhiElimination):
                in_ssa = False
            allocated = flow.checked(allocated, phase, in_ssa=in_ssa)
            (output / f"{stem}-{stage:02}-{phase.name}.txt").write_text(_lir(allocated))
            stage += 1
        allocated = qb_compile._drop_machine_side_entry(
            allocated,
            temporary_blocks,
            ordinary_entry,
            ordinary_fallback,
        )
        final = finalized(
            allocated,
            parameter_bytes=function.abi.parameter_bytes if function.abi is not None else 0,
        )
        final_body = qb_compile._address_values(qb_compile._source_instructions(final.body))
        (output / f"{stem}-{stage:02}-inline-x87.txt").write_text(_lir(final_body, final.callees))
    # The per-function LIR stages deliberately stop before the source ABI
    # envelope. Finish with the exact assembly model handed to OMF emission so
    # a showcase cannot hide runtime frame entry/exit, parameter cleanup,
    # module initialization, or source-data layout.
    (output / "99-emitted-asm.asm").write_text(_emitted_asm(program))
    return output


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--dialect", default="vbdos")
    parser.add_argument("--runtime", default="vbdos")
    parser.add_argument("--array-order", choices=("column-major", "row-major"), default="column-major")
    parser.add_argument("--huge-arrays", action="store_true")
    parser.add_argument("--checked-arrays", action="store_true")
    parser.add_argument("--mbf", action="store_true")
    parser.add_argument("--alternate-math", action="store_true")
    parser.add_argument("--include", action="append", default=[], type=Path)
    options = parser.parse_args(argv)
    output = options.output or Path("build/qbstages") / options.source.stem
    print(
        dumped(
            options.source,
            output,
            dialect=options.dialect,
            runtime=options.runtime,
            array_order=options.array_order,
            huge_arrays=options.huge_arrays,
            checked_arrays=options.checked_arrays,
            mbf=options.mbf,
            alternate_math=options.alternate_math,
            includes=tuple(options.include),
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
