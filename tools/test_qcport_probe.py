"""The probe the hourly sweep asks of a front-end compile at another build."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import qcport_probe as p  # noqa: E402


def compiler(tmp_path: Path, stderr: str, code: int) -> Path:
    tmp_path.mkdir(parents=True, exist_ok=True)
    for name in ("llrm-qb", "llrm-nib"):
        path = tmp_path / name
        (tmp_path / "stderr.txt").write_text(stderr)
        path.write_text(f"#!/bin/sh\ncat {tmp_path}/stderr.txt >&2\nexit {code}\n")
        path.chmod(0o755)
    return tmp_path


KIND = "panic crates/ir/llrm-mir/src/passes.rs:N:N: lsr added a memory operation"
PANIC = "thread 'main' panicked at crates/ir/llrm-mir/src/passes.rs:3037:9:\nlsr added a memory operation: the frozen globals-modref no longer covers the module"


def test_a_compile_that_panics_with_the_kind_is_red_and_one_that_compiles_is_not(tmp_path):
    red = compiler(tmp_path / "red", PANIC + "\n", 101)
    assert p.probe("matrix.bas", "-m32", "-O1", KIND, red) == 1
    green = compiler(tmp_path / "green", "", 0)
    assert p.probe("matrix.bas", "-m32", "-O1", KIND, green) == 0


def test_a_failure_of_another_kind_is_not_the_asked_one(tmp_path):
    other = compiler(tmp_path / "other", "llrm-c: a jump to L1_2, which is nowhere", 1)
    assert p.probe("matrix.bas", "-m32", "-O1", KIND, other) == 0


def test_a_program_the_front_end_set_does_not_have_exits_3(tmp_path):
    assert p.probe("no_such_program.bas", "-m32", "-O1", KIND, compiler(tmp_path, "", 0)) == 3
