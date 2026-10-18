"""The LLVM every comparison is made against, named in one place: the pinned build, its version in the output, and a refusal for any other.

The pinned build is LLVM 24.0.0git d1106deb71cc at ~/scratch/llvm-build. Two figures in the plan came from the distro's 20.1.8 on PATH (the clang column
of the per-pass table and of compile-gap's 557 M for d_faces -O2, where the pinned clang costs 415.5 M), because each tool chose its own `clang`.

    from llvmpin import resolve, header
    clang = resolve("clang", args.clang, args.allow_other_llvm)   # a path; SystemExit if it is not the pinned build
    print(header(clang))

A shell script takes `bin=$(python3 tools/llvmpin.py dir)`, or `python3 tools/llvmpin.py path opt`.
"""

from __future__ import annotations

import os
import sys
import shutil
import subprocess
from pathlib import Path

PINNED_BIN = Path.home() / "scratch/llvm-build/bin"
COMMIT = "d1106deb71cc"


def version(tool: str) -> str:
    """The first line of `tool --version` (clang names the commit, opt and llc the version)."""
    done = subprocess.run([tool, "--version"], capture_output=True, text=True)
    lines = (done.stdout or done.stderr).splitlines()
    return next((line.strip() for line in lines if "version" in line), lines[0].strip() if lines else "no version")


def pinned(name: str) -> Path:
    return PINNED_BIN / name


def resolve(name: str, override: str | Path | None = None, allow_other: bool = False) -> str:
    """The path of tool `name`: the pinned build's. `override` names another; it is refused unless `allow_other` (a flag the tool exposes) or it is the
    same build. Never PATH: a distro tool is found only when named."""
    if override is None or str(override) == name:
        if not pinned(name).is_file():
            raise SystemExit(f"{name}: the pinned LLVM ({PINNED_BIN}, {COMMIT}) is not built here; name one with its flag and --allow-other-llvm")
        return str(pinned(name))
    path = shutil.which(str(override)) or str(override)
    if allow_other or not pinned(name).is_file():
        return path
    if version(path) != version(str(pinned(name))):
        raise SystemExit(f"{path} is {version(path)}, not the pinned {version(str(pinned(name)))}: name it with --allow-other-llvm if that is the one to measure")
    return path


GCC_VERSION = "13.4.0"  # the plan's reference gcc (Ubuntu 13.4.0-4ubuntu1); gcc is the distro's only


def gcc(override: str | None = None, allow_other: bool = False) -> str:
    """The gcc on PATH (or `override`) when it is the reference version, else SystemExit unless `allow_other`."""
    path = shutil.which(override or "gcc") or (override or "gcc")
    if not allow_other and GCC_VERSION not in version(path):
        raise SystemExit(f"{path} is {version(path)}, not the pinned gcc {GCC_VERSION}: name it with --allow-other-llvm if that is the one to measure")
    return path


def missing() -> str | None:
    """Why the pinned toolchain is not here (the clang build, gcc of the reference version), else None."""
    if not pinned("clang").is_file():
        return f"the pinned clang ({pinned('clang')}) is not built here"
    if GCC_VERSION not in version(shutil.which("gcc") or "gcc"):
        return f"gcc is {version(shutil.which('gcc') or 'gcc')}, not {GCC_VERSION}"
    return None


def pin_required() -> bool:
    """A self-hosted runner (tinytim) has the pin: a test that needs it fails there when it is absent instead of skipping."""
    return os.environ.get("RUNNER_ENVIRONMENT") == "self-hosted"


def header(path: str) -> str:
    return f"{Path(path).name}: {path} ({version(path)})"


def main() -> int:
    if len(sys.argv) >= 2 and sys.argv[1] == "dir":
        print(PINNED_BIN)
        return 0
    if len(sys.argv) == 3 and sys.argv[1] == "path":
        print(gcc() if sys.argv[2] == "gcc" else resolve(sys.argv[2]))
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
