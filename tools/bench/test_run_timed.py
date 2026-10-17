"""A timed-only program is run and its output compared in the default bench: sized only, a miscompile reached no gate."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import bench  # noqa: E402

KEY = ("jpeg_single", "c", "O2")
SIZED = {"code_bytes": 10, "data_bytes": 0, "bss_bytes": 0}


def fake(monkeypatch, result):
    ran = []
    monkeypatch.setattr(bench, "time_programs", lambda exes, work, needs: ran.append(sorted(exes)) or {key: result for key in exes})
    return ran


def test_without_time_a_timed_only_program_is_run_and_marked_checked(monkeypatch, tmp_path):
    ran = fake(monkeypatch, {"cycles": 5, "ms": 1.0})
    out = {KEY: dict(SIZED), ("sieve", "c", "O2"): {"instructions": 9}}
    bench.run_timed(out, {KEY: Path("a.exe"), ("sieve", "c", "O2"): Path("b.exe")}, {KEY: {"files": (), "want": ["1"]}}, tmp_path, timing=False)
    assert ran == [[KEY]], "only the timed-only program (the one with an input) runs"
    assert out[KEY] == SIZED | {"checked": True} and "cycles" not in out[KEY], "no times without --time"
    assert out[("sieve", "c", "O2")] == {"instructions": 9}


def test_a_timed_only_program_that_prints_something_else_is_an_error_row(monkeypatch, tmp_path):
    fake(monkeypatch, {"error": "prints ['7'], want ['9']"})
    out = {KEY: dict(SIZED)}
    bench.run_timed(out, {KEY: Path("a.exe")}, {KEY: {"files": (), "want": ["9"]}}, tmp_path, timing=False)
    assert out[KEY] == {"error": "prints ['7'], want ['9']"}


def test_a_missing_corpus_leaves_the_row_unchecked_not_checked_or_failed_as_a_miscompile(monkeypatch, tmp_path):
    fake(monkeypatch, {"error": "skipped: corpus not here"})
    out = {KEY: dict(SIZED)}
    bench.run_timed(out, {KEY: Path("a.exe")}, {KEY: {"files": (), "want": []}}, tmp_path, timing=False)
    assert out[KEY] == SIZED | {"unchecked": "corpus not here"}
