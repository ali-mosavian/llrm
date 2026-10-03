"""bench/nbody_single's .out must not depend on rounding (#360).

Its first version (damping 1/16) was chaotic: a 1e-7 nudge to one start position moved the host's own
answer from 43832 to 82760, so -O1, which keeps float temporaries in the x87's extended precision,
printed 84266 and was called a miscompile. A benchmark whose answer is noise measures nothing.
"""

import subprocess
from pathlib import Path

import pytest

BENCH = Path(__file__).resolve().parents[1] / "bench" / "nbody_single"


@pytest.mark.parametrize("nudge", ["0", "1e-7f", "1e-6f"])
def test_the_host_prints_the_out_whatever_the_start_is_nudged_by(nudge, tmp_path):
    source = (BENCH / "nbody_single.c").read_text()
    start = "pos_x[body] = (float)(body * 7 - 15);"
    assert start in source, "premise: the start positions are where this test nudges them"
    source = source.replace(start, f"pos_x[body] = (float)(body * 7 - 15) + {nudge} * body;")
    source = source.replace("extern void report(long value);", "#include <stdio.h>\nvoid report(long v) { printf(\"%ld\\n\", v); }")
    (tmp_path / "n.c").write_text(source)
    subprocess.run(["gcc", "-O0", "-w", str(tmp_path / "n.c"), "-o", str(tmp_path / "n")], check=True)
    got = subprocess.run([str(tmp_path / "n")], capture_output=True, text=True, check=True).stdout
    assert got == (BENCH / "nbody_single.out").read_text()
