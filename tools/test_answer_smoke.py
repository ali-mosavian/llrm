"""tools/answer_smoke.py: a wrong answer turns the exit code red."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import answer_smoke  # noqa: E402


def bench(tmp_path: Path, expected: str) -> Path:
    (tmp_path / "p").mkdir()
    (tmp_path / "p" / "p.bas").write_text("PRINT 1\n")
    (tmp_path / "p" / "p.out").write_text(expected)
    return tmp_path


def test_a_wrong_answer_on_either_target_is_counted_and_a_right_one_is_not(tmp_path):
    ran = lambda source, target: ("ok", " 42\r\n")  # noqa: E731
    right = answer_smoke.check(("p",), ran, bench(tmp_path, "42\n"))
    assert [one.split()[0] for one in right] == ["ok", "ok"]
    wrong = answer_smoke.check(("p",), ran, tmp_path)
    assert wrong == right
    (tmp_path / "p" / "p.out").write_text("43\n")
    red = answer_smoke.check(("p",), ran, tmp_path)
    assert [one.split()[0] for one in red] == ["WRONG", "WRONG"], "a deliberately wrong expected output is red"


def test_a_program_that_did_not_build_or_end_is_not_a_right_answer(tmp_path):
    root = bench(tmp_path, "42\n")
    lines = answer_smoke.check(("p",), lambda source, target: ("timeout", ""), root)
    assert not any(one.startswith("ok ") for one in lines)
