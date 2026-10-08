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


def test_one_file_costing_much_more_fails_though_the_geomean_barely_moves():
    """66 files, one 10% dearer: the geomean moves 0.15%, which a geomean limit alone passes."""
    base = counts(**{f"p{i}": 1000 for i in range(66)})
    now = dict(base) | {"p0 -O2": 1100}
    _, bad = cc.compare(base, now)
    assert any("p0 -O2" in line for line in bad), bad


def test_every_file_a_little_dearer_fails_on_the_geomean():
    base = counts(**{f"p{i}": 1000 for i in range(66)})
    now = {k: v + 10 for k, v in base.items()}  # +1% each; none past the worst limit
    _, bad = cc.compare(base, now)
    assert any("geomean" in line for line in bad), bad


def test_the_same_counts_and_cheaper_ones_pass():
    base = counts(a=1000, b=2000)
    assert cc.compare(base, base)[1] == []
    assert cc.compare(base, {k: v // 2 for k, v in base.items()})[1] == []


def test_a_file_measured_on_one_side_only_fails_not_passes():
    base = counts(a=1000, b=2000)
    assert cc.compare(base, counts(a=1000))[1]
    assert cc.compare(counts(a=1000), base)[1]


def test_qcport_not_measured_is_said_and_not_a_failure():
    base = counts(a=1000) | counts(**{"qcport/host": 5000})
    lines, bad = cc.compare(base, counts(a=1000))
    assert bad == [] and any("qcport: not measured" in line for line in lines), lines


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
