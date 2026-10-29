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
    monkeypatch.setenv("LLRM_COUNTER", "perf")  # on a runner with no counters `auto` is callgrind, which never reads this fake
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


def test_the_gcc_comparison_leaves_out_benches_that_read_files():
    """stft_* include bench_io.h, which needs llrm's OS layer: the 66-program comparison compiled them as plain C and stopped on `bench_create`."""
    included = [name for name, source in cc.programs.sources().items() if "bench_io.h" in source.read_text(errors="replace")]
    assert included == []


def test_qcports_modules_are_compiled_as_borland_c_on_the_mirror_of_its_headers(tmp_path, monkeypatch):
    """The cost gate (run 2) gave QCport the Watcom row and -I qctc/inc: dos.h's ^Z and `_asm { mov sp_now, sp }` were errors and `measure` failed."""
    (tmp_path / "src/host").mkdir(parents=True)
    (tmp_path / "src/host/dbg.c").write_text("int x;\n")
    monkeypatch.setenv("QCPORT", str(tmp_path / "src"))
    monkeypatch.setenv("LLRM_BORLAND_INC", str(tmp_path / "bcinc"))
    flags = cc.files(tmp_path)["qcport/dbg"][1]
    assert "--target=x86_16-dos" in flags and "-fasm-blocks" in flags and "-D_asm=__asm" in flags
    assert flags[flags.index("-isystem") + 1] == str(tmp_path / "bcinc") and "-nostdinc" in flags
