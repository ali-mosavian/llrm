"""Dump every implemented modern-language frontend stage to text files."""

import argparse
import subprocess
from pathlib import Path

from qbopt import hir
from qbopt.backend import cpu as targets
from qbopt.frontend.modern import driver
from qbopt.frontend.qb import physicalize
from qbopt.frontend.modern import compile as modern


def _frontend_text(source: Path, option: str) -> str:
    """Run one diagnostic frontend boundary and return its complete output."""
    try:
        result = subprocess.run(
            (*driver.command(), option, str(source)),
            cwd=driver.ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        raise driver.FrontendError(f"could not start modern frontend: {error}") from error
    if result.returncode:
        message = result.stderr.strip() or f"llrm-modern exited with status {result.returncode}"
        raise driver.FrontendError(message)
    return result.stdout


def dumped(source: Path, output: Path) -> Path:
    """Write source, lexical, syntax, HIR, and semantic-MIR snapshots."""
    output.mkdir(parents=True, exist_ok=True)
    program = driver.parsed(source)
    lowered = modern.semantic_lowered(program)
    target = targets.profile("386")

    (output / "00-input.mod").write_text(source.read_text())
    (output / "01-tokens.txt").write_text(_frontend_text(source, "--tokens"))
    (output / "02-syntax.txt").write_text(_frontend_text(source, "--syntax"))
    (output / "03-hir.json").write_text(hir.encode(program, indent=2))
    mir_files = []
    number = 4
    for function, semantic in zip(program.modules[0].functions, lowered, strict=True):
        name = semantic.name.replace(".", "-")
        optimized = modern.optimized(program, function, semantic, target)
        physical = physicalize(program, function, optimized)
        optimized_physical = modern.optimized(
            program,
            function,
            physical.lowered,
            target,
            physical.calls,
        )
        stages = (
            ("source", semantic),
            ("optimized", optimized),
            ("physical", physical.lowered),
            ("optimized-physical", optimized_physical),
        )
        for stage, body in stages:
            filename = f"{number:02}-{name}-{stage}-mir.txt"
            (output / filename).write_text(hir.mir_text(body))
            mir_files.append(f"{filename}  {stage} MIR for {semantic.name}")
            number += 1

    files = [
        "00-input.mod       exact source presented to the frontend",
        "01-tokens.txt      lexer output with source positions",
        "02-syntax.txt      indentation-aware syntax tree",
        "03-hir.json        verified, source-neutral common HIR",
        *mir_files,
    ]
    (output / "README.txt").write_text(
        "Modern frontend stage dumps\n"
        "===========================\n\n" + "\n".join(files) + "\n\n"
        "Native compilation runs the common MIR fixed point before and after ABI\n"
        "physicalization, then continues through legalization, LIR, allocation, and emission.\n"
    )
    return output


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--output", type=Path)
    options = parser.parse_args(argv)
    output = options.output or Path("build/modernstages") / options.source.stem
    print(dumped(options.source, output))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
