"""The QB DOSBox runs depend on the emulated clock, not the host's: a busy host made them fail.

Under the load of a shared gate the emulator waits for a CPU, and two limits on the wall clock fired: the stall check
(no events for 1800 s: a launch that was only starved) and the 120 s allowed to a 50 ms probe job, which made every test
that needs virtual time fail in setup ("it has no `virtual time`"); LIB.EXE's 300 s did the same to the archive build. The
limits are CPU seconds of the emulator now, which load does not change."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402


def test_a_launch_that_waits_without_using_cpu_is_not_stalled(tmp_path):
    """A starved emulator writes nothing for a while and burns nothing; it was killed as stalled after `stall` wall seconds."""
    done = dosbatch._launch(["sh", "-c", "sleep 3"], tmp_path, None, 1 << 20, 1)
    assert done is None


def test_a_launch_that_spins_without_reporting_is_still_stalled(tmp_path):
    with pytest.raises(subprocess.TimeoutExpired):
        dosbatch._launch(["sh", "-c", "while :; do :; done"], tmp_path, None, 1 << 20, 1)


def probe_binary(tmp_path: Path, body: str) -> Path:
    fake = tmp_path / "fake-dosbox"
    fake.write_text(f"#!/bin/bash\n{body}\n")
    fake.chmod(0o755)
    return fake


REPORT = 'echo \'{"ev":"end","instructions":1000000}\' >&"$DOSRUN_FD"'


def test_a_probe_start_that_waited_for_a_cpu_still_finds_virtual_time(tmp_path):
    """The probe's 50 ms job was given 120 wall seconds, so a start-up under load said the binary had no virtual time."""
    assert dosbatch._probe_virtual_time(probe_binary(tmp_path, f"sleep 3\n{REPORT}"), 1) is None


def test_a_probe_job_that_burns_cpu_is_not_virtual_time(tmp_path):
    problem = dosbatch._probe_virtual_time(probe_binary(tmp_path, "while :; do :; done"), 1)
    assert problem and "CPU time" in problem


def test_the_archive_build_is_limited_by_the_emulators_cpu_time_not_the_wall_clock(tmp_path, monkeypatch):
    """LIB.EXE had 300 wall seconds; under load the archive every llrm test links failed to build, in setup."""
    import qbruntime

    monkeypatch.setattr(dosbatch, "DOSBOX", probe_binary(tmp_path, "while :; do :; done"))
    monkeypatch.setattr(dosbatch, "QB45", tmp_path)
    obj = tmp_path / "a.obj"
    obj.write_bytes(b"\x80")
    with pytest.raises(subprocess.TimeoutExpired):
        qbruntime.archive([obj], tmp_path / "out.lib", tmp_path / "work", seconds=1)


def test_the_gfx_benchmark_is_importable_beside_the_bench_tool_in_one_process():
    """tools/bench/bench.py and the gfx benchmark were both `bench`; in the gate's single pytest process the fixture's
    `import bench` returned the first, which has no `run`: 26 setup errors in test_qbgfxspeed."""
    import importlib.util

    root = Path(__file__).resolve().parent.parent
    spec = importlib.util.spec_from_file_location("bench", root / "bench" / "bench.py")
    sys.modules["bench"] = importlib.util.module_from_spec(spec)
    sys.path.insert(0, str(Path(__file__).parent / "gfxbench"))
    try:
        import gfxbench_run

        assert hasattr(gfxbench_run, "run")
    finally:
        sys.path.remove(str(Path(__file__).parent / "gfxbench"))
        del sys.modules["bench"]
