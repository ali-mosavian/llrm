"""Dump every implemented QB source-frontend stage to adjacent text files."""

import re
import argparse
from typing import cast
from pathlib import Path

from qbopt import hir
from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import masm
from qbopt.frontend.qb import parsed
from qbopt.frontend.qb import stage_text
from qbopt.objectfile.module import Space
from qbopt.frontend.qb import compile as qb_compile


def _source_text(path: Path) -> str:
    """Decode the physical source exactly as qbfront does before lexing."""
    raw = path.read_bytes().split(b"\x1a", 1)[0]
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError:
        return raw.decode("cp437")


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


def _source_globals(program: hir.Program, module: masm.Module) -> dict[str, tuple[int, str]]:
    """Return emitted source-global names with their exact zero-fill extent.

    This is deliberately a diagnostic-only view.  The frontend owns BASIC's
    spelling of source globals, while the shared MASM writer owns the bytes.
    Keeping that distinction here lets the showcase be readable without
    changing either the object writer or its backend-neutral data model.
    """
    globals_ = {}
    for source_module in program.modules:
        types = {one.id: one for one in source_module.types}
        for function in source_module.functions:
            for place in function.places:
                if place.storage is not hir.Storage.MODULE or place.extent is None or place.name.startswith("$"):
                    continue
                name = module.names.get((Space.SEGMENT, place.symbol))
                if name is None:
                    continue
                type_name = types[place.type].name
                globals_[name] = (place.extent, type_name)
    return globals_


def _zero_fill(size: int) -> str:
    """Spell initialized zero bytes compactly, preserving the emitted bytes."""
    match size:
        case 1:
            return "db 0"
        case 2:
            return "dw 0"
        case 4:
            return "dd 0"
        case 8:
            return "dq 0"
        case _:
            return f"db {size} dup (0)"


def _pretty_preamble(program: hir.Program, module: masm.Module) -> str:
    """Render the emitted data model as readable, byte-equivalent MASM."""
    globals_ = _source_globals(program, module)
    out = [".model medium", ".386", ""]
    out += [f"public {name}" for name in module.publics]
    out += ["", "; --------------------------------------------------------------------------", "; Data", ""]
    for segment, items in module.data:
        private = segment in module.private
        if out[-1]:
            out.append("")
        out.append(masm.SEGMENTS.get(segment, f"{segment} segment word public '{'FAR_DATA' if private else 'DATA'}'"))
        out += [f"extern {name}:byte" for name, kind in module.externs if kind == "byte"]
        source_heading = False
        index = 0
        while index < len(items):
            item = items[index]
            following = items[index + 1] if index + 1 < len(items) else None
            if isinstance(item, masm.Label) and item.name in globals_ and isinstance(following, bytes):
                extent, type_name = globals_[item.name]
                if len(following) == extent and not any(following):
                    if not source_heading:
                        out += ["", "    ; QB source globals: BC-compatible effective names", ""]
                        source_heading = True
                    out.append(f"{item.name:<20} {_zero_fill(extent):<16} ; {type_name}")
                    index += 2
                    continue
            match item:
                case bytes() if len(item) >= 4 and not any(item):
                    out.append(f"    {_zero_fill(len(item))}")
                case bytes():
                    out += [f"    {line}" for line in masm.datum(item)]
                case _:
                    out += masm.datum(item)
            index += 1
        if segment not in masm.SEGMENTS:
            out.append(f"{segment} ends")
            if not private:
                out.append(f"DGROUP group {segment}")
    out += [
        f"extern {name}:{'byte' if kind == 'far-byte' else kind}" for name, kind in module.externs if kind != "byte"
    ]
    out += [
        "",
        "; --------------------------------------------------------------------------",
        "; Code",
        f".code {module.code}",
    ]
    return "\n".join(out) + "\n"


def _display_assembly(text: str) -> str:
    """Align instructions and hide only display-only, unreferenced block labels."""
    lines = text.splitlines()
    referenced = {
        match.group(1) for line in lines if not line.endswith(":") for match in re.finditer(r"\b(L\d+_\d+)\b", line)
    }
    out = []
    for line in lines:
        heading = line.endswith(" proc far") or line.endswith(" proc near")
        label = line.endswith(":")
        name = line[:-1] if label else ""
        entry_label = bool(out and (out[-1].endswith(" proc far") or out[-1].endswith(" proc near")))
        if label and name.startswith("L") and not entry_label and name not in referenced:
            continue
        # Keep a procedure's entry label adjacent to its envelope: it makes
        # the runtime frame sequence easy to scan.  Any surviving internal
        # label starts a visually distinct basic block.
        if (heading or (label and not entry_label)) and out and out[-1]:
            out.append("")
        if heading:
            out += [
                "; --------------------------------------------------------------------------",
                f"; Procedure: {line.split(' proc ', 1)[0]}",
            ]
        if line.startswith("    "):
            match = re.fullmatch(r"    ([A-Za-z][A-Za-z0-9]*)\s+(.+)", line)
            if match is not None:
                line = f"    {match.group(1):<8}{match.group(2)}"
        out.append(line)
    return "\n".join(out) + "\n"


def _emitted_asm(program: hir.Program, module: masm.Module, *, pretty: bool = True) -> str:
    """Render the exact return cleanup carried by the emitted assembly model.

    The shared MASM diagnostic printer historically spells every far return as
    bare ``retf`` even when its semantics carry the immediate which the OMF
    writer correctly encodes. Keep this source-frontend showcase truthful
    without changing the shared backend.
    """
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

    # masm.text() uses the shared native frame shell. BASIC OMF emission uses
    # _basic_listing(), where B$ENRA/B$EXSA own that shell. Replace each
    # procedure with the listing which object_bytes() actually encodes.
    rendered = _pretty_preamble(program, module) if pretty else masm.text(module)
    if pretty:
        # The shared printer contributes the procedure envelopes below.  Its
        # data preamble has already been replaced by the readable equivalent.
        for number, procedure in enumerate(module.procedures):
            rendered += "\n".join(masm._procedure(procedure, module.names, number)) + "\n"
        rendered += "end\n"
    for number, procedure in enumerate(module.procedures):
        heading = f"{procedure.name} proc {'far' if procedure.far else 'near'}"
        ending = f"{procedure.name} endp"
        lines = [heading]
        for item in qb_compile._basic_listing(procedure, number):
            match item:
                case masm.Label(name=name):
                    lines.append(f"{name}:")
                case masm.Callee(code=code) if code:
                    lines += [f"    {line}" for line in masm._code(code)]
                case masm.Callee(name=name, far=far):
                    lines.append(f"    call {'far ptr ' if far else ''}{name}")
                case _:
                    lines += [f"    {line}" for line in masm._instruction(item, module.names, number)]
        lines.append(ending)
        rendered_lines = rendered.splitlines()
        try:
            start = rendered_lines.index(heading)
            stop = rendered_lines.index(ending, start + 1)
        except ValueError as error:
            raise ValueError(f"missing exact procedure envelope for {procedure.name}") from error
        rendered_lines[start : stop + 1] = lines
        rendered = "\n".join(rendered_lines) + "\n"

    current = None
    lines = []
    for line in rendered.splitlines():
        if line.endswith(" proc far") or line.endswith(" proc near"):
            current = line.split(" proc ", 1)[0]
        elif line.endswith(" endp"):
            current = None
        if current in cleanup and line.strip() == "retf":
            line = f"    retf {cleanup[current]}"
        lines.append(line)
    result = "\n".join(lines) + "\n"
    return _display_assembly(result) if pretty else result


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
    functions = tuple(function for module in program.modules for function in module.functions)
    (output / "00-input.bas").write_text(_source_text(source))
    numbers = {id(function): number for number, function in enumerate(functions, 1)}
    next_machine_stage = {id(function): 8 for function in functions}

    def observe(event: qb_compile.Stage) -> None:
        if event.name == "hir":
            (output / "01-hir.json").write_text(hir.encode(cast(hir.Program, event.value)))
            return
        if event.name == "emitted-assembly":
            # This is the assembly model object_bytes() is about to encode,
            # not a fresh assembled(program) diagnostic reconstruction.
            module = cast(masm.Module, event.value)
            (output / "99-emitted-asm.asm").write_text(_emitted_asm(program, module))
            (output / "99-emitted-asm.raw.asm").write_text(_emitted_asm(program, module, pretty=False))
            return
        if event.function is None:
            raise ValueError(f"stage {event.name} has no source function")
        number = numbers[id(event.function)]
        stem = f"{number:02}-{event.function.name}"
        match event.name:
            case "source-mir":
                path = output / f"{stem}-02-mir.txt"
                text = hir.mir_text(cast(hir.Lowered, event.value))
            case "optimized-mir":
                path = output / f"{stem}-03-optimized-mir.txt"
                text = hir.mir_text(cast(hir.Lowered, event.value))
            case "physical-mir":
                path = output / f"{stem}-04-physical-mir.txt"
                text = hir.mir_text(cast(hir.Lowered, event.value))
            case "optimized-physical-mir":
                path = output / f"{stem}-05-optimized-physical-mir.txt"
                text = hir.mir_text(cast(hir.Lowered, event.value))
            case "rotated-mir":
                path = output / f"{stem}-06-rotated-mir.txt"
                text = hir.mir_text(cast(hir.Lowered, event.value))
            case "initial-lir":
                path = output / f"{stem}-07-lir.txt"
                text = _lir(cast(lir.LirBody, event.value))
            case name if name.startswith("machine:"):
                stage = next_machine_stage[id(event.function)]
                next_machine_stage[id(event.function)] = stage + 1
                path = output / f"{stem}-{stage:02}-{name.removeprefix('machine:')}.txt"
                text = _lir(cast(lir.LirBody, event.value))
            case "final-lir":
                stage = next_machine_stage[id(event.function)]
                path = output / f"{stem}-{stage:02}-inline-x87.txt"
                text = _lir(cast(lir.LirBody, event.value), event.callees)
            case _:
                raise ValueError(f"unknown QB compiler stage {event.name}")
        path.write_text(text)

    qb_compile.object_bytes(program, source.name, observer=observe)
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
