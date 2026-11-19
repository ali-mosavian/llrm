#!/usr/bin/env python3
"""m32run.py BASEBIN NEWBIN LEVEL: each tests/run/c and bench program built with both dists at x86_32-linux, linked with a `report` shim, run natively;
prints how many programs moved and whether every moved program's output equals base's and its `.out`."""
import hashlib
import os
import subprocess
import sys
import tempfile
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import corpus_targets  # noqa: E402

TARGET = "x86_32-linux"
SHIM = '#include <stdio.h>\nvoid report(long v){printf("%ld\\n",v);}\n'


def link_command(clang: str, objects: list[str], out: str) -> list[str]:
    """Our -fno-pic objects need a non-PIE link: in clang's default PIE a call through the PLT (memset@plt) needs ebx = GOT, which they never set
    (spill_store_order and memfuncs_runtime segfaulted and read as base miscompiles)."""
    return [clang, "-m32", "-no-pie", *objects, "-o", out, "-lm"]


def build(dist: Path, level: str, src: Path, work: Path, shim: Path, clang: str):
    env = dict(os.environ, LLRM_CC_TARGET=TARGET, LLRM_LLC=str(dist / "llrm-llc"))
    obj = work / (dist.name + ".o")
    inc = ["-I" + str(ROOT / "bench"), "-I" + str(ROOT / "runtime/shared/posix")]
    if subprocess.run([str(dist / "llrm-cc"), "-" + level, "-c", "-o", str(obj), str(src), *inc], cwd=ROOT, env=env, capture_output=True).returncode:
        return None, None
    md5 = hashlib.md5(obj.read_bytes()).hexdigest()
    exe = work / (dist.name + ".exe")
    if subprocess.run(link_command(clang, [str(obj), str(shim)], str(exe)), capture_output=True).returncode:
        return md5, None
    return md5, exe


def output(exe: Path) -> bytes:
    try:
        return subprocess.run([str(exe)], capture_output=True, timeout=60).stdout
    except subprocess.TimeoutExpired:
        return b"TIMEOUT"


def main(argv: list[str]) -> int:
    base, new, level = Path(argv[0]), Path(argv[1]), argv[2]
    clang = subprocess.run([sys.executable, str(ROOT / "tools/pin_clang.py")], capture_output=True, text=True, check=True).stdout.strip()
    with tempfile.TemporaryDirectory(prefix="m32run-") as shim_dir:
        shim = Path(shim_dir) / "r.o"
        (Path(shim_dir) / "r.c").write_text(SHIM)
        subprocess.run([clang, "-m32", "-c", str(Path(shim_dir) / "r.c"), "-o", str(shim)], check=True)

        def one(src: Path) -> str:
            if subprocess.run([sys.executable, str(ROOT / "tools/corpus_targets.py"), "check", TARGET, str(src)], capture_output=True).returncode:
                return "na"
            with tempfile.TemporaryDirectory(prefix="m32run-") as t:
                (md5a, exea), (md5b, exeb) = build(base, level, src, Path(t), shim, clang), build(new, level, src, Path(t), shim, clang)
                if md5a is None or md5b is None:
                    return "nocompile"
                if exea is None or exeb is None:
                    return "nolink"
                if md5a == md5b:
                    return "unchanged"
                oa, ob = output(exea), output(exeb)
                expected = src.with_suffix(".out")
                return "moved-" + ("output-differs-from-base" if oa != ob else "equal-base") + ("" if not expected.exists() or ob == expected.read_bytes() else "-differs-from-out")

        srcs = sorted((ROOT / "tests/run/c").glob("*.c")) + sorted((ROOT / "bench").glob("*/*.c"))
        with ThreadPoolExecutor(int(os.environ.get("JOBS", "6"))) as ex:
            print(level, dict(Counter(ex.map(one, srcs))))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
