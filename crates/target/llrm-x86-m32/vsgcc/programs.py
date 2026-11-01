"""The 70 programs the vsgcc comparison compiles, and the flags llrm-c and llrm-cc compile them with."""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
sys.path.insert(0, str(REPO / "tools"))
import benchlist  # noqa: E402
# A bench program left out, and why. `stale_skips` fails when the reason no longer holds, so a program that starts building at
# m32, or stops existing, is not skipped for a name.
SKIPPED = {
    "huge": ("16-bit only: `__huge` data", "gcc_rejects"),
    "textfill": ("16-bit only: writes the PC text screen", "gcc_rejects"),
    "grep": ("timed only: no instruction count (bench.toml timed_only)", "timed_only"),
}
LLRM_FLAGS = ["-m32", "-mabi=sysv", "-march=i486"]  # build.sh's
CC_FLAGS = ["-mabi=sysv"]  # llrm-cc's: the target is a row, named by llrmbin.c_args(32)


def sources() -> dict[str, Path]:
    """Program name -> its source: bench/ and the x_ kernels written for the comparison."""
    found = {p.name: p / f"{p.name}.c" for p in benchlist.directories(REPO / "bench", parity=False)
             if p.name not in SKIPPED and "bench_io.h" not in (p / f"{p.name}.c").read_text(errors="replace")}  # files: no gcc build
    return found | {p.name: p / f"{p.name}.c" for p in sorted((HERE / "kernels").iterdir())}


def stale_skips() -> list[str]:
    """The skipped programs whose recorded reason no longer holds: gone, or building where they were said not to."""
    import subprocess
    bench = {p.name: p for p in benchlist.directories(REPO / "bench", parity=False)}
    stale = []
    for name, (reason, probe) in SKIPPED.items():
        if name not in bench:
            stale.append(f"{name}: no longer a benchmark ({reason})")
        elif probe == "timed_only":
            if "timed_only = true" not in (bench[name] / "bench.toml").read_text():
                stale.append(f"{name}: no longer timed only")
        elif probe == "gcc_rejects":
            done = subprocess.run(["gcc", "-m32", "-Dfar=", "-fsyntax-only", str(bench[name] / f"{name}.c")], capture_output=True)
            if done.returncode == 0:
                stale.append(f"{name}: gcc -m32 builds it now ({reason})")
    return stale


if __name__ == "__main__":
    print(" ".join(sources()))  # run.sh's list
