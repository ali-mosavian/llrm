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
        assert "mtriple" in text or "targetdesc" in text, f"{name}: no -mtriple anywhere"
        for number, line in enumerate(text.splitlines(), 1):
            if re.search(r'\[str\(PINNED\),|\[f?"\{?LLVM_BUILD', line) and not any(e in line for e in EXEMPT):
                assert "TARGET" in line or "mtriple" in line or "LLC" in line, f"{name}:{number}: {line.strip()[:120]}"


def test_the_triple_and_cpu_come_from_the_targets_description_and_the_default_is_the_widest(monkeypatch):
    """Four oracles and difftest each held the same i486 triple literal, and the target_facts ratchet refused the merge: one reader of datalayout.toml."""
    import targetdesc

    monkeypatch.delenv("LLRM_DIFFTEST_TARGET", raising=False)
    assert set(targetdesc.targets()) == {"m16", "m32", "m64"} and targetdesc.default() == "m64"
    assert targetdesc.triple_cpu() == ["-mtriple=x86_64-unknown-linux-gnu", "-mcpu=x86-64"]
    assert targetdesc.opt_args("m16")[:2] == ["-mtriple=i386-unknown-unknown-code16", "-mcpu=i486"] and "--data-layout=e-p:16:16" in targetdesc.opt_args("m16")[2]
    monkeypatch.setenv("LLRM_DIFFTEST_TARGET", "host")
    assert targetdesc.opt_args() == []
