"""Materialize generated QB source inputs without depending on ~/work."""

from __future__ import annotations

import argparse
from pathlib import Path

from tools.qbgen.verify import verify
from tools.qbgen.families import cases
from tools.qbgen.model import validate
from tools.qbgen.model import write_cases


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="write deterministic QB-family generated BASIC cases")
    parser.add_argument("output", type=Path, help="directory for 8.3 DOS BASIC source")
    parser.add_argument("--verify", action="store_true", help="also run the full parser/semantic/HIR sweep (slow)")
    arguments = parser.parse_args(argv)
    generated = cases()
    validate(generated)
    manifest = write_cases(arguments.output, generated)
    print(f"wrote {len(generated)} source cases to {manifest}")
    if arguments.verify:
        verify(generated)
        print(f"verified {len(generated)} source cases through the QB frontend")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
