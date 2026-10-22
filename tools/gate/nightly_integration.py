#!/usr/bin/env python3
"""The nightly full run of the root crate's integration targets (126 binaries, 275 s to run after a 277 s rebuild at load 14: too slow for the landing gate).

    nightly_integration.py [--cargo-arg ARG...]

Prints every red test as `integration <target>::<test>` (the names the landing gate compares), and the targets with no result. Exit 1 if any red test is not listed in
tools/xfail.toml as `integration <target>::<test>`.
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent))
import xfail  # noqa: E402
from buildslot import slotted  # noqa: E402


from parity_smoke import red_tests  # noqa: E402


def main(argv: list[str]) -> int:
    done = subprocess.run(slotted(["cargo", "test", "--release", "--no-fail-fast", "-p", "llrm", "--tests", *argv]), cwd=ROOT, capture_output=True, text=True)
    red = red_tests(done.stdout + done.stderr)
    unexpected = [one for one in red if one not in xfail.load()]
    print("\n".join(red) or "no red integration test")
    print(f"{len(red)} red, {len(unexpected)} not in tools/xfail.toml")
    return 1 if unexpected else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
