import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "selfhosted"))
os.environ["PAT_FILE"] = str(Path(__file__))  # any readable file: the module reads the token at import

import supervisor  # noqa: E402


def test_a_runner_that_dies_at_once_makes_the_slot_wait():
    """A bad podman flag made every start fail in milliseconds; the slot registered a runner per spin (30 offline ones in a minute)."""
    assert supervisor.backoff(0.2) > 0
    assert supervisor.backoff(3600) == 0


def test_only_registrations_offline_at_two_beats_in_a_row_are_reaped():
    """A runner just started is offline for a moment; only one that stays so is dead."""
    runners = [
        {"id": 1, "name": "tinytim-1-aa", "status": "offline"},
        {"id": 2, "name": "tinytim-2-bb", "status": "offline"},
        {"id": 3, "name": "tinytim-3-cc", "status": "online"},
        {"id": 4, "name": "someone-else", "status": "offline"},
    ]
    assert supervisor.stale(runners, {"tinytim-1-aa", "tinytim-3-cc", "someone-else"}) == [1]


def test_cpus_are_the_slot_units_not_podmans():
    """Rootless podman put containers in the service's own cgroup and ignored --cpuset-cpus/--cpus: nproc read 32 in every container.
    The units pin them (AllowedCPUs from install.sh), so the CPU limit lives in the cgroup the container is in."""
    src = Path(supervisor.__file__).read_text()
    assert "\"--cpuset-cpus" not in src and "--cpus=" not in src
    assert "AllowedCPUs=" in (Path(supervisor.__file__).parent / "install.sh").read_text()


def _cpus(spec):
    return {c for part in spec.split(",") for c in range(int(part.split("-")[0]), int(part.split("-")[-1]) + 1)}


def test_a_runner_is_whole_physical_cores_in_one_ccd_and_slots_do_not_overlap():
    """Four threads spread over four cores run at a half of what two whole cores give; two SMT siblings of different cores or a core pair split
    across the CCDs' L3 slow the jobs. Siblings are (c, c+16); the CCDs are cores 0-7 and 8-15; cores 0, 1, 8, 9 are left to interactive work."""
    seen = set()
    for spec in supervisor.CPUSETS:
        cpus = _cpus(spec)
        assert len(cpus) == 4 and {c % 16 for c in cpus} == {c % 16 for c in cpus if c < 16} | {c - 16 for c in cpus if c >= 16}
        assert all((c + 16 in cpus) if c < 16 else (c - 16 in cpus) for c in cpus)
        assert len({(c % 16) // 8 for c in cpus}) == 1
        assert not (cpus & seen) and not (cpus & {0, 1, 8, 9, 16, 17, 24, 25})
        seen |= cpus
    assert supervisor.SLOTS == 6


def test_the_cpusets_match_the_machines_sibling_pairs():
    """The pairs above are tinytim's; where the machine says otherwise (or is another machine) the test is not about it."""
    import pytest

    try:
        sib = lambda c: _cpus(Path(f"/sys/devices/system/cpu/cpu{c}/topology/thread_siblings_list").read_text().strip())
        if sib(0) != {0, 16}:
            pytest.skip("not the 16-core, 32-thread host the sets are for")
    except OSError:
        pytest.skip("no sysfs topology")
    for spec in supervisor.CPUSETS:
        for c in _cpus(spec):
            assert sib(c) <= _cpus(spec)


def test_the_workspace_is_the_hosted_one():
    """The target cache from a hosted build holds /home/runner/work paths in llrm-c; on tinytim's /home/runner/_work every wccq spawn failed
    "No such file or directory" (30 python-group test files red)."""
    assert supervisor.WORK == "work"


def test_the_image_replaces_the_perf_wrapper_in_place():
    """/usr/bin/perf is a wrapper that wants tools of the host's exact kernel; a job's PATH found it before /usr/local/bin and the scaling
    tests (12 files) read "perf counted nothing"."""
    text = (Path(__file__).parent / "selfhosted/Containerfile").read_text()
    assert "dpkg-divert --local --rename --add /usr/bin/perf" in text


def test_a_runner_without_instruction_counters_is_named_not_pooled(monkeypatch):
    """Perf counters on tinytim were inferred, never read: a slot whose container lacks them would take cost and perf jobs and fail them. The probe's output says which."""
    assert supervisor.counters_missing("1234567,,instructions:u,4000,100.00,,\n") is None
    assert "not supported" in supervisor.counters_missing("<not supported>,,instructions:u,0,100.00,,\n")
    assert supervisor.counters_missing("0,,instructions:u,0,100.00,,\n")
    assert "perf_event_open" in supervisor.counters_missing("Error:\nperf_event_open(..., 0) failed: Operation not permitted\n")
    assert supervisor.counters_missing("")
    ran = []
    monkeypatch.setattr(supervisor.subprocess, "run", lambda command, **kw: ran.append(command) or type("D", (), {"stdout": "<not counted>,,instructions:u,0,0,,\n", "stderr": ""})())
    assert supervisor.preflight(3) is False
    assert ran[0][1:1 + len(supervisor.container_flags())] == supervisor.container_flags(), "the probe must run as a job does"


def test_a_slot_that_fails_its_preflight_registers_no_runner(monkeypatch):
    """The probe gates the pool: no JIT config is requested for a slot without counters."""
    asked = []
    monkeypatch.setattr(supervisor, "preflight", lambda n: False)
    monkeypatch.setattr(supervisor, "api", lambda *a, **k: asked.append(a))

    def stop(_seconds):
        raise KeyboardInterrupt

    monkeypatch.setattr(supervisor.time, "sleep", stop)
    try:
        supervisor.slot(1)
    except KeyboardInterrupt:
        pass
    assert asked == []
