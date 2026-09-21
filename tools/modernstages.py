"""Dump every implemented modern-language frontend stage to text files."""

import argparse
import subprocess
from pathlib import Path

from qbopt import hir
from qbopt.frontend.modern import driver


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
        message = result.stderr.strip() or f"modernfront exited with status {result.returncode}"
        raise driver.FrontendError(message)
    return result.stdout


def dumped(source: Path, output: Path) -> Path:
    """Write source, lexical, syntax, HIR, and semantic-MIR snapshots."""
    output.mkdir(parents=True, exist_ok=True)
    program = driver.parsed(source)
    lowered = hir.lower(program)

    (output / "00-input.mod").write_text(source.read_text())
    (output / "01-tokens.txt").write_text(_frontend_text(source, "--tokens"))
    (output / "02-syntax.txt").write_text(_frontend_text(source, "--syntax"))
    (output / "03-hir.json").write_text(hir.encode(program, indent=2))
    for number, function in enumerate(lowered, 1):
        name = function.name.replace(".", "-")
        (output / f"{number + 3:02}-{name}-mir.txt").write_text(hir.mir_text(function))

    files = [
        "00-input.mod       exact source presented to the frontend",
        "01-tokens.txt      lexer output with source positions",
        "02-syntax.txt      indentation-aware syntax tree",
        "03-hir.json        verified, source-neutral common HIR",
        *(
            f"{number + 3:02}-{function.name.replace('.', '-')}-mir.txt  semantic MIR for {function.name}"
            for number, function in enumerate(lowered, 1)
        ),
    ]
    (output / "README.txt").write_text(
        "Modern frontend stage dumps\n"
        "===========================\n\n" + "\n".join(files) + "\n\n"
        "The frontend currently stops at semantic MIR. There is no modern-language\n"
        "target ABI, physical MIR, LIR, register allocation, or emitted assembly yet.\n"
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
