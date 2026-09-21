"""Execute a modern-language source file through the verified HIR boundary."""

import sys
import argparse
from pathlib import Path

from qbopt.hir import execute
from qbopt.frontend.modern import driver


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("arguments", nargs="*", type=int)
    parser.add_argument("--entry")
    parser.add_argument("--show-return", action="store_true")
    options = parser.parse_args(argv)
    program = driver.parsed(options.source)
    functions = program.modules[0].functions
    entry = options.entry or ("main" if any(one.name == "main" for one in functions) else functions[0].name)
    result = execute.run(program, entry, tuple(options.arguments))
    print(result.output, end="")
    if options.show_return:
        print(f"return: {result.value}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
