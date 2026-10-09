"""compile-cost.py: a rise in what the compiler spends is caught, and a counter that reads zero is not a pass."""
import importlib.util
import os
import stat
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("compile_cost", HERE / "compile-cost.py")
cc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cc)


def counts(**per_file):
    return {f"{name} {level}": n for name, n in per_file.items() for level in cc.LEVELS}


def test_a_counter_that_reads_zero_is_unavailable_not_a_count(tmp_path, monkeypatch):
    """A VM with no counters prints `<not supported>` or 0; reading it as 0 instructions made every compile free."""
    fake = tmp_path / "perf"
    fake.write_text('#!/bin/sh\nwhile [ "$1" != -o ]; do shift; done\necho "0,,instructions:u,0,0.00,," > "$2"\n')
    fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
    monkeypatch.setenv("PATH", f"{tmp_path}{os.pathsep}{os.environ['PATH']}")
    with pytest.raises(cc.NoCounter):
        cc.instructions(["true"])


def test_the_counter_counts_work_not_time():
    """The same command twice is the same count to within a hair: the property the limits rest on."""
    if not Path("/usr/bin/perf").exists():
        pytest.skip("no perf")
    try:
        a, b = (cc.instructions([sys.executable, "-S", "-c", "sum(range(200000))"]) for _ in range(2))
    except cc.NoCounter:
        pytest.skip("no counter")
    assert abs(a / b - 1) < 0.02 and a > 1_000_000
