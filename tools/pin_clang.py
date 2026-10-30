#!/usr/bin/env python3
"""The pin's clang, from one setting (LLRM_PIN_CLANG, default
<path to the pin build>/bin/clang). Prints its path; its --version goes to stderr.
Refuses any build that is not the pin's commit."""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def pin() -> str:
    """The pin's sha12: line 1 of toolchain/clang/pin (TOOLCHAIN_PIN names another file, as toolchain/clangbuild.sh reads it)."""
    file = Path(os.environ.get("TOOLCHAIN_PIN") or ROOT / "toolchain/clang/pin")
    return file.read_text().split()[0][:12]


def path() -> str:
    """The pin's clang, checked: exits with the reason when it does not run or is not the pin's commit. Its --version line goes to stderr."""
    clang = os.environ.get("LLRM_PIN_CLANG", str(Path.home() / "scratch/llvm-build/bin/clang"))
    try:
        version = subprocess.run([clang, "--version"], capture_output=True, text=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError) as error:
        sys.exit(f"pin_clang: {clang} does not run: {error}")
    if pin() not in version:
        sys.exit(f"pin_clang: {clang} is not the pin ({pin()}): {version.splitlines()[0]}")
    print(version.splitlines()[0], file=sys.stderr)
    return clang


def main():
    print(path())


if __name__ == "__main__":
    main()
