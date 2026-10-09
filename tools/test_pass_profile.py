"""pass-profile.py sums a step's own instructions over the files, and refuses CPU time for a count."""
import collections
import importlib.util
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("pass_profile", HERE / "pass-profile.py")
profile = importlib.util.module_from_spec(spec)
spec.loader.exec_module(profile)


def test_steps_are_summed_per_group_and_qcport_is_kept_apart():
    got = profile.summed({"qcport/a": {"mir gvn": 3.0, "isel": 1.0}, "qcport/b": {"mir gvn": 2.0}, "fib": {"mir gvn": 7.0}})
    assert got["QCport"] == collections.Counter({"mir gvn": 5.0, "isel": 1.0}) and got["programs"] == collections.Counter({"mir gvn": 7.0})
    assert "QCport" not in profile.summed({"fib": {"isel": 1.0}})


def test_the_table_is_shares_of_the_group_largest_first():
    lines = profile.table("g", collections.Counter({"a": 1.0, "b": 3.0}), 5)
    assert lines[0] == "g: 4 Minstr" and "75.0%" in lines[1] and lines[1].split()[1] == "b" and "25.0%" in lines[2]


def test_cpu_time_rows_are_no_counter(tmp_path):
    fake = tmp_path / "llrm-c"
    fake.write_text("#!/bin/sh\necho '[instr]  1.000 Mcpu-ns own  1.000 Mcpu-ns total  1x mir gvn' >&2\n")
    fake.chmod(0o755)
    with pytest.raises(profile.compile_cost.NoCounter):
        profile.own_work(fake, [], tmp_path / "a.c", "-O2")
