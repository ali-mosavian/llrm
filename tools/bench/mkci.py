#!/usr/bin/env python3
"""The expected output of a timed-only program on the first ci_bytes of its corpus, from the host's gcc (not from llrm).

    tools/bench/mkci.py grep           writes bench/grep/grep.ci.out

The program's C source is built with `gcc -m32` (its `long` is 32 bits as under DOS) beside a harness that stands for the DOS runtime's `report` and
`input_read`. Checked first against the full corpus: its output must equal NAME.out, or the harness is not an oracle.
"""

from __future__ import annotations

import re
import sys
import tomllib
import tempfile
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools/dosbatch"))

import corpus  # noqa: E402

HARNESS = """#include <stdio.h>
extern long KERNEL(long *checksum);
static FILE *in;
void report(long v) { printf("%ld\\n", v); }
int input_read(char *b, int n) { return (int)fread(b, 1, n, in); }
int main(int argc, char **argv) { long s; in = fopen(argv[1], "rb"); long l = KERNEL(&s); report(l); report(s); return 0; }
"""


FILE_HARNESS = """#include <stdio.h>
#include <fcntl.h>
#include <unistd.h>
extern long KERNEL(void);
void report(long v) { printf("%ld\\n", v); }
long llrm_os_open(const char *name, int mode) { return open(name, O_RDONLY); }
long llrm_os_create(const char *name) { return open(name, O_WRONLY | O_CREAT | O_TRUNC, 0644); }
long llrm_os_read(long h, unsigned char *b, int n) { return read(h, b, n); }
long llrm_os_write_file(long h, const unsigned char *b, int n) { return write(h, b, n); }
long llrm_os_close(long h) { return close(h); }
int main(void) { report(KERNEL()); return 0; }
"""


def host_output(name: str, data: bytes, kernel: str, tmp: Path, file_name: str | None = None) -> str:
    """gcc's output on `data`. A program that reads and writes files by name (bench_io.h: stft_fixed) finds `data` as `file_name` in its working directory."""
    source = (ROOT / "bench" / name / f"{name}.c").read_text().replace("int main(void)", "int main_unused(void)").replace('"../bench_io.h"', '"bench_io.h"')
    (tmp / "kernel.c").write_text(source)
    (tmp / "harness.c").write_text((FILE_HARNESS if file_name else HARNESS).replace("KERNEL", kernel))
    (tmp / (file_name or "input")).write_bytes(data)
    subprocess.run(["gcc", "-m32", "-O1", "-w", "-I", str(ROOT / "bench"), "-I", str(ROOT / "runtime/shared/posix"), "kernel.c", "harness.c", "-o", "host"], cwd=tmp, check=True)
    return subprocess.run(["./host", *([] if file_name else ["input"])], cwd=tmp, check=True, capture_output=True, text=True).stdout


def main() -> int:
    name = sys.argv[1]
    settings = tomllib.loads((ROOT / "bench" / name / "bench.toml").read_text())
    kernel = settings["region"]["c"]
    header = re.search(r"^// data: @(\w+)", (ROOT / "bench" / name / f"{name}.c").read_text(), re.M)
    wanted = header.group(1) if header else "dickens"
    file_name = wanted.upper() if "bench_io.h" in (ROOT / "bench" / name / f"{name}.c").read_text() else None
    full = corpus.path(wanted).read_bytes()
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        if host_output(name, full, kernel, tmp, file_name).split() != (ROOT / "bench" / name / f"{name}.out").read_text().split():
            raise SystemExit(f"{name}: the host build does not reproduce {name}.out on the full corpus: not an oracle")
        out = host_output(name, full[: settings["ci_bytes"]], kernel, tmp, file_name)
    (ROOT / "bench" / name / f"{name}.ci.out").write_text(out)
    print(f"bench/{name}/{name}.ci.out: {out.split()}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
