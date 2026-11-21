"""tipred: a red test is traced to its commit, a refused or broken tick says so, a dead allow-list name is an error."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pytest  # noqa: E402
import tipred  # noqa: E402


def git(repo, *args):
    return subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True, text=True).stdout.strip()


@pytest.fixture
def repo(tmp_path):
    git(tmp_path, "init", "-q", "-b", "main")
    git(tmp_path, "config", "user.email", "t@t")
    git(tmp_path, "config", "user.name", "t")
    for number in range(6):
        (tmp_path / "state").write_text("red\n" if number >= 3 else "green\n")
        (tmp_path / f"f{number}").write_text(str(number))
        git(tmp_path, "add", "-A")
        git(tmp_path, "commit", "-q", "-m", f"c{number}")
    return tmp_path


def test_a_test_that_goes_red_at_a_commit_is_traced_to_that_commit(repo):
    """Silent until the gate found it at a PR: the monitor names the first red commit of the range."""
    commits = git(repo, "rev-list", "--reverse", "HEAD").split()
    first = tipred.bisect(commits, lambda c: tipred.red_at(str(repo), c, ["sh", "-c", "! grep -q red state"]))
    assert first == commits[3]


def test_a_range_that_ends_green_names_no_commit():
    assert tipred.bisect(["a", "b", "c"], lambda c: False) is None
    assert tipred.bisect([], lambda c: True) is None


def test_the_bisect_stops_at_its_step_cap():
    probes = []
    commits = [str(n) for n in range(1000)]
    tipred.bisect(commits, lambda c: probes.append(c) or int(c) >= 500)
    assert len(probes) <= tipred.BISECT_STEPS + 1


def test_a_tick_that_cannot_run_is_reported_not_silent(tmp_path, monkeypatch):
    """Load over the limit: the tick names the reason to every recipient; silence would read as green."""
    sent = []
    monkeypatch.setattr(tipred.os, "getloadavg", lambda: (50.0, 50.0, 50.0))
    monkeypatch.setattr(tipred, "lane_locked", lambda *a: False)
    state = tmp_path / "state.json"
    tipred.tick(str(tmp_path), state, False, reporter_for=lambda s: tipred.Reporter(s, lambda to, text: sent.append((to, text)) or True))
    assert [to for to, _ in sent] == list(tipred.RECIPIENTS)
    assert "load1m 50" in sent[0][1]


def test_refusal_reasons():
    assert tipred.refusal(10, 400 << 30, False) is None
    assert "load1m" in tipred.refusal(40, 400 << 30, False)
    assert "free" in tipred.refusal(10, 100 << 30, False)
    assert "lock" in tipred.refusal(10, 400 << 30, True)


def test_a_held_lane_lock_is_seen_and_given_back(tmp_path):
    import fcntl

    path = tmp_path / "lane.lock"
    with open(path, "w") as holder:
        fcntl.flock(holder, fcntl.LOCK_EX)
        assert tipred.lane_locked((str(path),))
    assert not tipred.lane_locked((str(path),))


def test_an_allow_list_name_that_matches_no_test_is_an_error():
    listed = {"llrm-m68k": {"bswap::every_form", "ccr::every_row"}}
    assert tipred.check_env_list([("llrm-m68k", "ccr::every_row"), ("llrm-m68k", "renamed")], listed) == ["llrm-m68k renamed"]


def test_a_failed_test_and_a_broken_build_are_told_apart():
    ok = "test a::b ... ok\ntest a::c ... FAILED\ntest d ... FAILED\n"
    assert tipred.failed_tests(ok) == (["a::c", "d"], False)
    assert tipred.failed_tests("error[E0063]: could not compile `x` (lib test) due to 2 errors\n") == ([], True)


def test_a_change_reaches_the_crates_that_depend_on_it():
    root = "/r"
    packages = [
        {"name": "llrm-a", "manifest_path": "/r/crates/a/Cargo.toml", "dependencies": [], "root": root},
        {"name": "llrm-b", "manifest_path": "/r/crates/b/Cargo.toml", "dependencies": [{"name": "llrm-a"}], "root": root},
        {"name": "llrm-c", "manifest_path": "/r/crates/c/Cargo.toml", "dependencies": [{"name": "llrm-b"}], "root": root},
        {"name": "llrm-d", "manifest_path": "/r/crates/d/Cargo.toml", "dependencies": [], "root": root},
    ]
    assert tipred.affected_crates(["crates/a/src/x.rs"], packages) == ["llrm-a", "llrm-b", "llrm-c"]
    assert tipred.affected_crates([], packages, everything=True) == ["llrm-a", "llrm-b", "llrm-c", "llrm-d"]


def test_a_key_is_sent_once():
    sent = []
    reporter = tipred.Reporter({}, lambda to, text: sent.append(to) or True)
    reporter.report("red:x:1", "t")
    reporter.report("red:x:1", "t")
    assert sent == list(tipred.RECIPIENTS)


def test_a_failed_send_is_retried_next_tick():
    state = {}
    tipred.Reporter(state, lambda to, text: False).report("k", "t")
    assert state["sent"] == []
    assert state["unsent"] == {"k": "t"}


def test_a_report_with_no_bus_is_a_file_per_recipient(tmp_path, monkeypatch):
    """No bus a script can reach: each recipient gets a file a session forwards, naming who it is for."""
    monkeypatch.setenv("TIPRED_OUTBOX", str(tmp_path))
    assert tipred.Reporter({}).report("k", "x is red") is None
    files = sorted(tmp_path.iterdir())
    assert len(files) == len(tipred.RECIPIENTS)
    assert {f.read_text().splitlines()[0] for f in files} == {f"to: {to}" for to in tipred.RECIPIENTS}


def test_no_personal_path_is_written_into_the_script():
    """tipred.py once carried `Path.home() / "scratch/w9/..."` and a gcc-m68k path: one person's
    machine in the repo, which the path scan refused. Every such setting comes from the unit's env."""
    source = (Path(__file__).resolve().parent / "tipred.py").read_text()
    assert "Path.home()" not in source and "scratch/" not in source


def test_an_unset_setting_is_a_named_error(monkeypatch):
    """With TIPRED_OUTBOX unset a report was written under a guessed path; it now stops and says which variable."""
    monkeypatch.delenv("TIPRED_OUTBOX", raising=False)
    with pytest.raises(SystemExit) as stop:
        tipred.send("a@b", "text")
    assert "TIPRED_OUTBOX" in str(stop.value)
