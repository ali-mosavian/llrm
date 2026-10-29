"""The 70 programs the vsgcc comparison compiles, and the flags llrm-cc compiles them with."""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
sys.path.insert(0, str(REPO / "tools"))
import benchlist  # noqa: E402
NOT_PROGRAMS = {"readme.md", "parity", "huge", "textfill", "grep"}  # 16-bit only, no input, or timed only (run.sh)
LLRM_FLAGS = ["--target=x86_32-wat", "-march=i486", "-mabi=sysv"]  # build.sh's
CC_FLAGS = ["-mabi=sysv"]  # llrm-cc's: the target is a row, named by llrmbin.c_args(32)


def sources() -> dict[str, Path]:
    """Program name -> its source: bench/ and the x_ kernels written for the comparison."""
    found = {p.name: p / f"{p.name}.c" for p in benchlist.directories(REPO / "bench", parity=False)
             if p.name not in NOT_PROGRAMS and "bench_io.h" not in (p / f"{p.name}.c").read_text(errors="replace")}  # files: no gcc build
    return found | {p.name: p / f"{p.name}.c" for p in sorted((HERE / "kernels").iterdir())}


if __name__ == "__main__":
    print(" ".join(sources()))  # run.sh's list
