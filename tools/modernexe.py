#!/usr/bin/env python3
"""Build one modern-language module as a small real-mode DOS executable."""

import os
import shutil
import argparse
import tempfile
import subprocess
from pathlib import Path
from dataclasses import dataclass

from dosbox import launch
from configs import CONFIGS
from dosbox import dos_file
from dosbox import read_dos

from qbopt import flow
from qbopt.model.passes import O2
from qbopt.backend import omfwrite
from qbopt.model.passes import Options
from qbopt.frontend.modern import driver
from qbopt.cfront import compile as cfront
from qbopt.frontend.modern import compile as modern

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "runtime" / "nib"
DEFAULT_JWASM = Path.home() / "work" / "other" / "d32x" / "toolchains" / "native" / "bin" / "jwasm"


class BuildError(RuntimeError):
    """The host or DOS toolchain could not produce an executable."""


@dataclass(frozen=True, slots=True)
class Built:
    executable: Path
    size: int
    output: str | None


def _jwasm() -> Path:
    configured = os.environ.get("JWASM")
    found = configured or shutil.which("jwasm")
    path = Path(found) if found else DEFAULT_JWASM
    if not path.is_file():
        raise BuildError("jwasm is not installed; set JWASM to its executable")
    return path


def _assemble(assembler: Path, source: Path, output: Path) -> None:
    done = subprocess.run(
        [str(assembler), "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{output}", str(source)],
        capture_output=True,
        text=True,
    )
    if done.returncode:
        raise BuildError(f"assembling {source.name} failed:\n{done.stdout}{done.stderr}")


def build(
    source: Path,
    output: Path,
    *,
    entry: str = "main",
    run: bool = False,
    options: Options = O2,
) -> Built:
    """Compile, link and optionally run ``source`` through the minimal runtime."""
    source = source.resolve()
    output = output.resolve()
    config = CONFIGS["v-g3"]
    if not config.available:
        raise BuildError("the configured DOS linker toolchain is unavailable")
    if not cfront.WCCQ.is_file():
        raise BuildError("owshim/bin/wccq is missing; run owshim/build.sh")

    with tempfile.TemporaryDirectory(prefix="qbopt-modern-") as scratch_name:
        scratch = Path(scratch_name)
        program = driver.parsed(source)
        (scratch / "PROGRAM.OBJ").write_bytes(modern.written(program, entry=entry, source=source, options=options))

        runtime_source = RUNTIME / "rt.c"
        runtime_stream = cfront.recorded(runtime_source, [])
        runtime_module = cfront.assembled(runtime_stream, "rt", optimise=True, options=options)
        (scratch / "RT.OBJ").write_bytes(omfwrite.written(runtime_module, runtime_source.name))

        assembler = _jwasm()
        _assemble(assembler, RUNTIME / "start.asm", scratch / "START.OBJ")
        _assemble(assembler, RUNTIME / "dos.asm", scratch / "DOS.OBJ")

        commands = [
            f"{config.link} START.OBJ+PROGRAM.OBJ+RT.OBJ+DOS.OBJ, PROGRAM.EXE,,; > LINK.OUT",
        ]
        if run:
            commands.append("PROGRAM.EXE > PROGRAM.OUT")
        result = launch(scratch, config.mount, commands, timeout=30)
        if not result.finished or result.timed_out:
            raise BuildError(f"DOS linker/run did not finish: {result}")
        report = read_dos(scratch, "LINK.OUT")
        if "error l" in report.lower() or "unresolved external" in report.lower():
            raise BuildError(f"linking failed:\n{report}")
        linked = dos_file(scratch, "PROGRAM.EXE")
        if linked is None:
            raise BuildError(f"linker produced no executable:\n{report}")

        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(linked, output)
        captured = read_dos(scratch, "PROGRAM.OUT") if run else None
        return Built(output, output.stat().st_size, captured)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("-o", "--output", type=Path)
    parser.add_argument("--entry", default="main")
    parser.add_argument("--run", action="store_true", help="run under DOSBox-X and print stdout")
    flow.level_option(parser)
    options = parser.parse_args(argv)
    output = options.output or options.source.with_suffix(".exe")
    try:
        made = build(options.source, output, entry=options.entry, run=options.run, options=options.options)
    except (BuildError, driver.FrontendError, cfront.hir.Unsupported) as error:
        parser.error(str(error))
    print(f"{made.executable} ({made.size} bytes)")
    if made.output is not None:
        print(made.output, end="" if made.output.endswith("\n") else "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
