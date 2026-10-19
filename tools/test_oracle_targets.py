"""Every pinned opt/llc an oracle runs is told llrm's triple and cpu: without a `target triple` opt costs for the host (x86-64)."""
from __future__ import annotations

import re
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
ORACLES = ["unroll_oracle.py", "rotate_oracle.py", "simpleloopunswitch_oracle.py", "loops_oracle.py", "unrollfull_oracle.py", "loopdeletion_oracle.py", "switchcg_oracle.py"]
# opt/llc calls that read no target-dependent answer: a version probe.
EXEMPT = ("--version",)


def test_every_oracle_opt_or_llc_command_carries_a_triple():
    """inlinediff priced 30 GEPs at 5 each with opt's host default (370 vs 215 at i486); the unroll, rotate and loops oracles had the same hole."""
    for name in ORACLES:
        text = (TOOLS / name).read_text()
        assert "mtriple" in text, f"{name}: no -mtriple anywhere"
        for number, line in enumerate(text.splitlines(), 1):
            if re.search(r'\[str\(PINNED\),|\[f?"\{?LLVM_BUILD', line) and not any(e in line for e in EXEMPT):
                assert "TARGET" in line or "mtriple" in line or "LLC" in line, f"{name}:{number}: {line.strip()[:120]}"
