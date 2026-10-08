"""The 66 programs the vsgcc comparison compiles, and the flags llrm-c compiles them with."""
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
NOT_PROGRAMS = {"readme.md", "parity", "huge", "textfill", "grep"}  # 16-bit only, no input, or timed only (run.sh)
LLRM_FLAGS = ["-m32", "-mabi=sysv", "-march=i486"]  # build.sh's


def sources() -> dict[str, Path]:
    """Program name -> its source: bench/ and the x_ kernels written for the comparison."""
    found = {p.name: p / f"{p.name}.c" for p in sorted((REPO / "bench").iterdir()) if p.is_dir() and p.name not in NOT_PROGRAMS}
    return found | {p.name: p / f"{p.name}.c" for p in sorted((HERE / "kernels").iterdir())}
