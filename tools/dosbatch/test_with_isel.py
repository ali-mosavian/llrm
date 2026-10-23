"""LLRM_ISEL puts `--isel=` after llrm-c, in the commands run_tests builds."""

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dosbatch  # noqa: E402


def test_the_variable_selects_the_selector_for_llrm_c_only(monkeypatch):
    monkeypatch.setenv("LLRM_ISEL", "shadow")
    assert dosbatch.with_isel(["/b/llrm-c", "a.c", "-O2"]) == ["/b/llrm-c", "--isel=shadow", "a.c", "-O2"]
    assert dosbatch.with_isel(["/b/jwasm", "a.asm"]) == ["/b/jwasm", "a.asm"]


def test_without_the_variable_nothing_changes(monkeypatch):
    monkeypatch.delenv("LLRM_ISEL", raising=False)
    assert dosbatch.with_isel(["/b/llrm-c", "a.c"]) == ["/b/llrm-c", "a.c"]
