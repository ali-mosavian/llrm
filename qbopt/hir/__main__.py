"""Validate, normalize, or inspect a frontend-produced HIR document."""

import argparse
from pathlib import Path

from qbopt.hir.lower import lower
from qbopt.hir.codec import decode
from qbopt.hir.codec import encode
from qbopt.hir.dump import mir_text


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m qbopt.hir")
    parser.add_argument("input", type=Path)
    output = parser.add_mutually_exclusive_group()
    output.add_argument("--canonical", action="store_true", help="write canonical HIR JSON")
    output.add_argument("--mir", action="store_true", help="write the semantic MIR projection")
    options = parser.parse_args(arguments)
    program = decode(options.input.read_text())
    if options.canonical:
        print(encode(program), end="")
    elif options.mir:
        for body in lower(program):
            print(mir_text(body), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
