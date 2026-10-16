"""levels_time.counted: one count of instructions, by perf where it reads one and by callgrind where it does not."""
import shutil
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tools"))

import levels_time  # noqa: E402

needs_valgrind = pytest.mark.skipif(shutil.which("valgrind") is None, reason="valgrind is not installed")


def test_the_counter_is_named_by_the_environment_and_a_wrong_name_is_refused(monkeypatch):
    monkeypatch.setenv("LLRM_COUNTER", "callgrind")
    assert levels_time.backend() == "callgrind"
    monkeypatch.setenv("LLRM_COUNTER", "qemu")
    with pytest.raises(SystemExit):
        levels_time.backend()


@needs_valgrind
def test_callgrind_counts_a_command_the_same_every_time_and_leaves_its_stderr_alone(monkeypatch):
    """Hosted VMs have no hardware counters: measure was out of every CI run. The count is the compile's own, not valgrind's messages."""
    monkeypatch.setenv("LLRM_COUNTER", "callgrind")
    runs = [levels_time.counted(["sh", "-c", "echo said >&2"]) for _ in range(3)]
    assert len({count for count, _, _ in runs}) == 1 and runs[0][0] > 1000
    assert runs[0][2].stderr == "said\n"


@needs_valgrind
def test_a_command_that_fails_is_counted_and_reports_its_status(monkeypatch):
    monkeypatch.setenv("LLRM_COUNTER", "callgrind")
    count, _, done = levels_time.counted(["sh", "-c", "exit 3"])
    assert count > 0 and done.returncode == 3


@needs_valgrind
def test_a_stored_measurement_names_its_counter_and_the_passes_are_not_measured_without_the_compiler_counter(monkeypatch):
    """Counts of callgrind and perf differ by 1.1% on one compile: a measurement of one is no base for the other. The compiler counts a
    step itself with perf_event_open, which callgrind does not give: the step costs are left out, not read as CPU time."""
    import measure
    import scaling_gate

    monkeypatch.setenv("LLRM_COUNTER", "callgrind")
    one = measure.method()
    monkeypatch.setenv("LLRM_COUNTER", "perf")
    assert measure.method() != one
    monkeypatch.setenv("LLRM_COUNTER", "callgrind")
    assert scaling_gate.measure_passes(1) == {}
