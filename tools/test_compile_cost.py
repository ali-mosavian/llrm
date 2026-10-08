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


def test_the_same_counts_pass_and_cheaper_ones_fail_until_the_baseline_is_refreshed():
    base = counts(a=1000, b=2000)
    assert cc.compare(base, base)[1] == []
    assert cc.compare(base, {k: v // 2 for k, v in base.items()})[1]


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


def test_a_drop_past_the_noise_fails_and_says_to_refresh():
    """A PR that cut 5% and left the baseline alone let the next ones give it back unseen."""
    base = counts(**{f"p{i}": 1000 for i in range(66)})
    _, bad = cc.compare(base, {k: v - 50 for k, v in base.items()})
    assert any("faster: refresh the baseline" in line and "geomean" in line for line in bad), bad
    _, bad = cc.compare(base, dict(base) | {"p0 -O2": 900})
    assert any("p0 -O2" in line and "faster" in line for line in bad), bad


def test_refresh_rewrites_the_files_that_moved_and_then_the_comparison_passes():
    base = counts(a=1000, b=2000, c=3000)
    now = counts(a=1000, b=1500, c=3001) | {"d -O2": 7}
    new = cc.refreshed(base, now)
    assert new["b -O2"] == 1500 and new["d -O2"] == 7  # moved, added
    assert new["c -O2"] == 3000 and new["a -O2"] == 1000  # within noise: left, so the diff shows only what moved
    assert cc.compare(new, now)[1] == []


def test_refresh_without_qcport_keeps_its_entries():
    base = counts(a=1000) | counts(**{"qcport/x": 500})
    new = cc.refreshed(base, counts(a=900))
    assert new["qcport/x -O2"] == 500 and new["a -O2"] == 900


def test_refresh_moves_everything_when_small_shifts_add_up_past_the_limit():
    base = counts(**{f"p{i}": 10000 for i in range(66)})
    now = {k: v + 40 for k, v in base.items()}  # +0.4% each: under MOVED, over the geomean limit
    assert cc.compare(base, now)[1]
    assert cc.compare(cc.refreshed(base, now), now)[1] == []
