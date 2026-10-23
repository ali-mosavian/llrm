#!/usr/bin/env python3
"""The pin's clang, from one setting (LLRM_PIN_CLANG, default
<path to the pin build>/bin/clang). Prints its path; its --version goes to stderr.
Refuses any build that is not the pin's commit."""
import os
import subprocess
import sys
from pathlib import Path

PIN = "d1106deb71cc"


def main():
    clang = os.environ.get("LLRM_PIN_CLANG", str(Path.home() / "scratch/llvm-build/bin/clang"))
    try:
        version = subprocess.run([clang, "--version"], capture_output=True, text=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError) as error:
        sys.exit(f"pin_clang: {clang} does not run: {error}")
    if PIN not in version:
        sys.exit(f"pin_clang: {clang} is not the pin ({PIN}): {version.splitlines()[0]}")
    print(version.splitlines()[0], file=sys.stderr)
    print(clang)


if __name__ == "__main__":
    main()
