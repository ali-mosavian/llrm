"""Every cargo that parity_smoke and prepush spawn runs under buildslot; BUILDSLOT_BYPASS=1 is the landing script's way out."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent / "gate"))
import buildslot as bs  # noqa: E402
import parity_smoke as ps  # noqa: E402
import prepush  # noqa: E402


def test_a_cargo_argv_or_shell_string_is_wrapped_and_anything_else_is_not(monkeypatch):
    monkeypatch.delenv("BUILDSLOT_BYPASS", raising=False)
    tool = bs.tool()
    assert bs.slotted(["cargo", "test", "-q"]) == [tool, "cargo", "test", "-q"]
    assert bs.slotted(["git", "status"]) == ["git", "status"]
    assert bs.slotted("cargo fmt --check && cargo test -q") == f"{tool} cargo fmt --check && {tool} cargo test -q"
    assert bs.slotted("python3 x.py") == "python3 x.py"
    monkeypatch.setenv("BUILDSLOT_BYPASS", "1")
    assert bs.slotted(["cargo", "test"]) == ["cargo", "test"] and bs.slotted("cargo test") == "cargo test"


def test_the_cargo_commands_parity_smoke_spawns_are_all_wrapped(monkeypatch, tmp_path):
    """The smoke ran `cargo check --workspace --all-targets` and the ratchets, unslotted, from several sessions at once: the host ran out of cores."""
    monkeypatch.delenv("BUILDSLOT_BYPASS", raising=False)
    seen = []

    class Done:
        returncode, stdout, stderr = 0, "", ""

    monkeypatch.setattr(ps.subprocess, "run", lambda command, *a, **k: (seen.append(command), Done())[1])
    ps.workspace_check(tmp_path)
    assert ps.ratchets(root=tmp_path) == ""
    assert seen and all(Path(c[0]).name == "buildslot" and c[1] == "cargo" for c in seen), [c[:3] for c in seen]


def test_prepush_runs_its_cargo_steps_under_buildslot(monkeypatch):
    monkeypatch.delenv("BUILDSLOT_BYPASS", raising=False)
    seen = []

    class Done:
        returncode, stdout, stderr = 0, "", ""

    monkeypatch.setattr(prepush.subprocess, "run", lambda command, *a, **k: (seen.append(command), Done())[1])
    prepush.run_all({"test x": "cargo test --release -q --test x", "fmt": "python3 tools/fmt.sh"}, {}, jobs=1)
    assert any(isinstance(c, str) and "buildslot cargo test" in c for c in seen) and "python3 tools/fmt.sh" in seen, seen


def test_the_landing_runs_cargo_unslotted_with_no_bypass_in_the_callers_env_and_others_stay_slotted(monkeypatch):
    """Batches of the gate took ~22 minutes, mostly waiting for a slot behind other sessions' builds: the runner shells never set BUILDSLOT_BYPASS."""
    monkeypatch.delenv("BUILDSLOT_BYPASS", raising=False)
    monkeypatch.delenv(bs.LANDING, raising=False)
    monkeypatch.setattr(bs, "tool", lambda: "/x/buildslot")
    assert bs.slotted(["cargo", "build"]) == ["/x/buildslot", "cargo", "build"]
    with bs.landing():
        assert bs.slotted(["cargo", "build"]) == ["cargo", "build"]
        assert bs.slotted("cargo test && cargo check") == "cargo test && cargo check"
        assert os.environ[bs.LANDING] == "1"  # the smoke it starts inherits it
    assert bs.slotted(["cargo", "build"]) == ["/x/buildslot", "cargo", "build"]


def test_parity_squash_main_runs_every_command_inside_the_landing(monkeypatch):
    import parity_squash

    seen = []
    monkeypatch.setattr(parity_squash, "dispatch", lambda: seen.append(os.environ.get(bs.LANDING)) or 0)
    monkeypatch.delenv(bs.LANDING, raising=False)
    assert parity_squash.main() == 0 and seen == ["1"] and bs.LANDING not in os.environ


def test_a_build_slot_caps_cargo_jobs_unless_the_caller_sets_them(tmp_path):
    """Four slots of cargo at every core (32 rustc of 1-1.7 GB each) ran the host out of memory: systemd-oomd killed whole sessions at 19:58 and 20:13-20:17."""
    script = str(Path(__file__).resolve().parent / "buildslot")
    run = lambda env: subprocess.run([script, "sh", "-c", "echo $CARGO_BUILD_JOBS"], capture_output=True, text=True, env=env).stdout.strip()  # noqa: E731
    base = {**os.environ, "BUILDSLOT_DIR": str(tmp_path)}
    default = run({k: v for k, v in base.items() if k != "CARGO_BUILD_JOBS"})
    assert default == "8", default
    assert run({**base, "CARGO_BUILD_JOBS": "3"}) == "3"


def test_a_test_that_allocates_past_the_cap_dies_alone(tmp_path):
    """A 175 GB test binary (a Reassociate test) took the host to 1 GB free and systemd-oomd killed whole sessions: a runaway test must die by itself."""
    testcap = str(Path(__file__).resolve().parent / "testcap")
    grab = ["python3", "-c", "import sys; b = bytearray(int(sys.argv[1]) << 30); print('ok')"]
    run = lambda gb: subprocess.run([testcap, *grab, str(gb)], capture_output=True, text=True, env={**os.environ, "TEST_MEM_CAP_GB": "1"})  # noqa: E731
    assert run(0).stdout.strip() == "ok" and run(0).returncode == 0
    over = run(4)
    assert over.returncode != 0 and "ok" not in over.stdout and "MemoryError" in over.stderr, (over.returncode, over.stderr[-200:])


def test_a_child_that_needs_more_than_the_cap_can_lift_it_for_itself(tmp_path):
    """qb_dirty's ThreadSanitizer run maps 35 TB of shadow: under testcap's hard 16 GB limit it died with `can not mmap the shadow memory` and the test was moved to nightly."""
    testcap = str(Path(__file__).resolve().parent / "testcap")
    grab = ["python3", "-c", "b = bytearray(4 << 30); print('ok')"]
    done = subprocess.run([testcap, "prlimit", "--data=unlimited", *grab], capture_output=True, text=True, env={**os.environ, "TEST_MEM_CAP_GB": "1"})
    assert done.stdout.strip() == "ok", (done.returncode, done.stderr[-200:])


def test_buildslot_hands_cargo_the_test_cap_unless_the_caller_has_a_runner(tmp_path):
    script = str(Path(__file__).resolve().parent / "buildslot")
    show = lambda env: subprocess.run([script, "sh", "-c", "echo $CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER"], capture_output=True, text=True, env=env).stdout.strip()  # noqa: E731
    base = {**os.environ, "BUILDSLOT_DIR": str(tmp_path)}
    base.pop("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER", None)
    assert show(base).endswith("/testcap"), show(base)
    assert show({**base, "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER": "mine"}) == "mine"


def test_only_four_builds_hold_a_slot_at_once(tmp_path):
    """Eight slots ran the host at load 83-100 of 32 cores with ~23 sessions building (2026-10-27) and starved the landing runner: four slots by default, a fifth command waits."""
    import time

    script = str(Path(__file__).resolve().parent / "buildslot")
    log = tmp_path / "starts"
    env = {k: v for k, v in {**os.environ, "BUILDSLOT_DIR": str(tmp_path / "slots")}.items() if k != "BUILDSLOTS"}
    procs = [subprocess.Popen([script, "sh", "-c", f"date +%s.%N >> {log}; sleep 2"], env=env) for _ in range(5)]
    for p in procs:
        p.wait(timeout=60)
    starts = sorted(float(x) for x in log.read_text().split())
    assert len(starts) == 5 and starts[4] - starts[0] >= 1.8 and starts[3] - starts[0] < 1.0, starts


def test_waiters_get_a_slot_in_arrival_order_and_a_priority_ticket_jumps_the_queue(tmp_path):
    """A dist build holding the shared dist lock waited 44 minutes for a slot while short tests kept taking it: random polling is not a queue."""
    import time

    script = str(Path(__file__).resolve().parent / "buildslot")
    log = tmp_path / "order"
    env = {**os.environ, "BUILDSLOT_DIR": str(tmp_path / "slots"), "BUILDSLOTS": "1", "BUILDSLOT_POLL": "0.1"}
    holder = subprocess.Popen([script, "sh", "-c", "sleep 1.5"], env=env)
    time.sleep(0.3)
    waiters = []
    for name, extra in (("first", {}), ("second", {}), ("urgent", {"BUILDSLOT_PRIORITY": "0"})):
        waiters.append(subprocess.Popen([script, "sh", "-c", f"echo {name} >> {log}; sleep 0.2"], env={**env, **extra}))
        time.sleep(0.2)
    for p in (holder, *waiters):
        p.wait(timeout=60)
    assert log.read_text().split() == ["urgent", "first", "second"], log.read_text()
