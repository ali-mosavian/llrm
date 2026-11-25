#!/usr/bin/env python3
"""pgo_train.py LLRM_CC [--list]: the PGO training run of tools/pgo_dist.sh. Compiles every tests/run/c program except HELD at -O1, -O2 and -Os
as x86_32 i486 (the caller sets LLVM_PROFILE_FILE). HELD and the QuickrBasic files are what the gain is measured on, so nothing here trains on them."""
import os, re, subprocess, sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HELD = {"bit_test.c", "memory_expansion.c", "long_compare_chain.c", "os_execute.c", "shl_long_by_one.c"}
LEVELS = ("O1", "O2", "Os")


def programs() -> list[Path]:
    return [p for p in sorted((ROOT / "tests/run/c").glob("*.c")) if p.name not in HELD]


def own_flags(src: Path) -> list[str]:
    first = src.read_text(errors="replace").split("\n")[0]
    m = re.match(r"//\s*flags:\s*(.*)", first)
    toks = m.group(1).split("|")[0].split() if m else []
    return [t for t in toks if not t.startswith(("-O", "-m32", "-m64", "-march", "-mtune"))]


def compile_one(job: tuple[str, Path, str]) -> bool:
    cc, src, level = job
    obj = f"/tmp/pgo-train-{os.getpid()}-{abs(hash((src, level)))}.o"
    cmd = [cc, "--target=x86_32-linux", "-march=i486", *own_flags(src), "-Dfar=", "-I", str(ROOT / "bench"), "-I", str(ROOT / "runtime/shared/posix"),
           f"-{level}", "-c", "-o", obj, str(src)]
    try:
        return subprocess.run(cmd, capture_output=True, timeout=600).returncode == 0
    finally:
        Path(obj).unlink(missing_ok=True)


if __name__ == "__main__":
    if "--list" in sys.argv:
        print("\n".join(p.name for p in programs()))
        sys.exit(0)
    env = {"LLRM_VERIFY": "0"}
    os.environ.update(env)
    cc = sys.argv[1]
    with ThreadPoolExecutor(os.cpu_count() // 4 or 1) as pool:
        ok = list(pool.map(compile_one, [(cc, p, l) for p in programs() for l in LEVELS]))
    print(f"pgo_train: {sum(ok)} of {len(ok)} compiles")
    sys.exit(0 if sum(ok) * 10 >= len(ok) * 8 else "pgo_train: fewer than 80% of the training compiles worked: the profile would not be representative")
