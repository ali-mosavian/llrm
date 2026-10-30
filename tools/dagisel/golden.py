"""Regenerates the SelectionDAG builder goldens: the pin's "Initial selection
DAG" dumps for each `.ll` under selectiondag/golden, from the assert build
(`~/scratch/llvm-build-assert`, or $LLC). Rust's `dump::normalized` compares.

    python3 tools/dagisel/golden.py
"""
import os
import re
import subprocess
from pathlib import Path

GOLDEN = Path("crates/codegen/llrm-codegen/src/backend/selectiondag/golden")
LLC = os.environ.get("LLC") or str(Path.home() / "scratch/llvm-build-assert/bin/llc")


def initial_dags(ll: Path) -> str:
    run = subprocess.run(
        [LLC, "-O2", "-mattr=-sse", "-disable-cgp", "-disable-lsr", "-relocation-model=static", "-debug-only=isel", "-o", "/dev/null", str(ll)],
        capture_output=True, text=True, check=True,
    )
    keep, out = False, []
    for line in run.stderr.splitlines():
        if line.startswith("Initial selection DAG"):
            keep = True
        elif re.match(r"(Optimized lowered|Type-legalized|Legalized|Optimized legalized)", line):
            keep = False
        if keep:
            out.append(line)
    return "\n".join(out) + "\n"


def case_clusters(ll: Path) -> str:
    """`name: clusters` for each switch the pin lowers."""
    run = subprocess.run(
        [LLC, "-O2", "-mattr=-sse", "-disable-cgp", "-disable-lsr", "-debug-only=isel", "-o", "/dev/null", str(ll)],
        capture_output=True, text=True, check=True,
    )
    out, name = [], None
    for line in run.stderr.splitlines():
        if line.startswith("=== "):
            name = line[4:].strip()
        elif line.startswith("Case clusters: "):
            out.append(f"{name}: {line[len('Case clusters: '):]}")
    return "\n".join(out) + "\n"


for ll in sorted(GOLDEN.glob("*.ll")):
    if ll.stem == "switches":
        ll.with_suffix(".pin.txt").write_text(case_clusters(ll))
    else:
        ll.with_suffix(".pin.txt").write_text(initial_dags(ll))
    print("wrote", ll.with_suffix(".pin.txt"))
