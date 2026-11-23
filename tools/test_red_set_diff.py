"""red_set_diff.py refuses a log whose reds are a missing build product."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "red_set_diff.py"

# A run whose 269 red names never ran the compiler.
MISSING = """---- test_size_keeps_sahf stdout ----

thread 'test_size_keeps_sahf' panicked at crates/driver/llrm-cc/tests/m32_x87_status_word.rs:32:5:
llrm-cc: /work/target/release/llrm-llc: No such file or directory (os error 2)
test test_size_keeps_sahf ... FAILED
"""
REAL = "test a_real_red ... FAILED\n"


def run(tmp: Path, tip: str, head: str) -> subprocess.CompletedProcess[str]:
    (tmp / "tip.log").write_text(tip)
    (tmp / "head.log").write_text(head)
    return subprocess.run(
        [sys.executable, str(SCRIPT), str(tmp / "tip.log"), str(tmp / "head.log")], capture_output=True, text=True
    )


def test_a_log_of_missing_build_products_is_refused_not_diffed(tmp_path: Path) -> None:
    done = run(tmp_path, REAL, MISSING)
    assert done.returncode == 2, done.stdout
    assert "instrument defect" in done.stdout


def test_a_name_only_in_the_head_is_printed(tmp_path: Path) -> None:
    done = run(tmp_path, "", REAL)
    assert (done.returncode, done.stdout.split()) == (1, ["a_real_red"])


def test_equal_sets_pass(tmp_path: Path) -> None:
    assert run(tmp_path, REAL, REAL).returncode == 0
