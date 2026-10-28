"""`uv run --project tools python -m pytest tools/test_datalayout_copies.py`

A target's datalayout is stated once, in its `datalayout.toml`. `tests/datalayout_copies.baseline` lists the copies left per file
(`count path`); it only shrinks: a file over its count, or absent from it, fails, and so does a file under its count until the
baseline is lowered (`DATALAYOUT_BLESS=1`). The goal is an empty baseline.
"""
import os
from pathlib import Path

import datalayout_sync as sync

BASELINE = sync.ROOT / "tests" / "datalayout_copies.baseline"


def baseline() -> dict[str, int]:
    if not BASELINE.exists():
        return {}
    rows = (line.split(" ", 1) for line in BASELINE.read_text().splitlines() if line and not line.startswith("#"))
    return {path: int(count) for count, path in rows}


def problems(found: dict[str, int], allowed: dict[str, int]) -> list[str]:
    wrong = []
    for path, n in sorted(found.items()):
        if path not in allowed:
            wrong.append(f"{path}: {n} copies of a datalayout, none allowed: read it from the target's datalayout.toml")
        elif n > allowed[path]:
            wrong.append(f"{path}: {n} copies, was {allowed[path]}")
        elif n < allowed[path]:
            wrong.append(f"{path}: {n} copies, baseline says {allowed[path]}: lower it (DATALAYOUT_BLESS=1)")
    wrong += [f"{path}: none left, remove it from the baseline (DATALAYOUT_BLESS=1)" for path in sorted(allowed) if path not in found]
    return wrong


def test_no_file_holds_more_copies_of_a_datalayout_than_its_baseline():
    found = sync.copies()
    if os.environ.get("DATALAYOUT_BLESS"):
        BASELINE.write_text("# Copies of a target datalayout left per file: count path. Only shrinks.\n" + "".join(f"{n} {path}\n" for path, n in sorted(found.items())))
        return
    assert problems(found, baseline()) == []


def test_a_second_copy_in_a_new_file_and_an_unlowered_baseline_both_fail():
    assert problems({"a.rs": 1}, {}) == ["a.rs: 1 copies of a datalayout, none allowed: read it from the target's datalayout.toml"]
    assert problems({"a.rs": 2}, {"a.rs": 1}) == ["a.rs: 2 copies, was 1"]
    assert problems({"a.rs": 1}, {"a.rs": 2}) == ["a.rs: 1 copies, baseline says 2: lower it (DATALAYOUT_BLESS=1)"]
    assert problems({}, {"a.rs": 1}) == ["a.rs: none left, remove it from the baseline (DATALAYOUT_BLESS=1)"]


def test_the_scan_counts_the_copies_in_a_tree(tmp_path: Path):
    layout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32"
    (tmp_path / "crates/target/llrm-x86-m16/src/machines").mkdir(parents=True)
    (tmp_path / "crates/target/llrm-x86-m16/src/machines/datalayout.toml").write_text(f'datalayout = "{layout}"\n')
    (tmp_path / "t.rs").write_text(f'target datalayout = "{layout}"\n"{layout}-ni:2:3"\n')
    (tmp_path / "crates/opt/llrm-analysis/corpus/emitted").mkdir(parents=True)
    (tmp_path / "crates/opt/llrm-analysis/corpus/emitted/g.ll").write_text(f'target datalayout = "{layout}"\n')
    (tmp_path / "f.mir").write_text(f'target datalayout = "{layout}"\n')
    import subprocess
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(["git", "add", "-A"], cwd=tmp_path, check=True)
    assert sync.copies(tmp_path) == {"t.rs": 2}, "neither the generated corpus nor .ll/.mir fixtures are counted: ~500 rows no one could shrink"
