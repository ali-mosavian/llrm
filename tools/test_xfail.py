"""The XFAIL list: a listed failure is expected, a listed pass fails the gate, an unlisted failure fails it."""
from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_smoke as ps  # noqa: E402
import xfail  # noqa: E402

LISTED = {"unit crate::t_a": {"id": "unit crate::t_a", "issue": 7, "owner": "o", "date": "2026-10-19"}}


def failure_output(*names: str) -> str:
    return "failures:\n\nfailures:\n" + "".join(f"    {name}\n" for name in names) + "\ntest result: FAILED.\n"


@pytest.fixture
def unit(monkeypatch):
    monkeypatch.setattr(ps, "selection", lambda: {"units": {"crate": {"tests": [["t_a", 0.1], ["t_b", 0.1]]}}})
    monkeypatch.setattr(xfail, "load", lambda path=xfail.PATH: LISTED)

    def run(code, output):
        monkeypatch.setattr(ps, "run_cut", lambda command, seconds, cwd, env=None: (code, output))
        return ps.unit_step("crate", 10.0)(Path("."))

    return run


def test_a_listed_test_that_fails_is_not_a_failure(unit):
    verdict, detail, *_ = unit(101, failure_output("t_a"))
    assert verdict == "ok" and "1 expected failures" in detail


def test_a_listed_test_that_passes_fails_the_gate_and_names_its_entry(unit):
    verdict, detail, names = unit(0, "test result: ok. 2 passed\n")
    assert verdict == "fail" and "remove the entry" in detail and names == ["xpass crate::t_a"]


def test_an_unlisted_failure_fails_the_gate_even_beside_a_listed_one(unit):
    verdict, detail, names = unit(101, failure_output("t_a", "t_b"))
    assert verdict == "fail" and names == ["unit crate::t_b"]


def test_a_listed_test_the_run_did_not_include_says_nothing():
    unexpected, expected, passing = xfail.classify(set(), {"unit crate::t_b"}, LISTED)
    assert not unexpected and not expected and not passing


def test_an_entry_without_an_owner_or_issue_or_listed_twice_is_refused(tmp_path):
    path = tmp_path / "x.toml"
    path.write_text('[[xfail]]\nid = "run a"\nissue = 3\ndate = "d"\n')
    with pytest.raises(SystemExit, match="lacks owner"):
        xfail.load(path)
    path.write_text('[[xfail]]\nid = "run a"\nissue = 3\nowner = "o"\ndate = "d"\n[[xfail]]\nid = "run a"\nissue = 3\nowner = "o"\ndate = "d"\n')
    with pytest.raises(SystemExit, match="listed twice"):
        xfail.load(path)
    assert xfail.issue_of("run a", {"run a": {"issue": 3}}) == "#3" and xfail.issue_of("run b", {}) is None


def run_integration(monkeypatch, output):
    monkeypatch.setattr(xfail, "load", lambda path=xfail.PATH: {"integration t::t_a": {}})
    done = type("Done", (), {"stdout": output, "returncode": 0})()
    return ps.integration_failures(["t"], run=lambda *a, **k: done)


def test_a_listed_integration_test_that_passes_is_an_xpass_and_one_that_fails_is_excused(monkeypatch):
    """Only `integration` ids that fail were excused; one that was fixed stayed listed, silently, because nothing read its pass."""
    passing = "     Running tests/t.rs (target/release/deps/t-1)\ntest t_a ... ok\ntest t_b ... ok\n"
    assert run_integration(monkeypatch, passing) == {"xpass integration t::t_a"}
    failing = "     Running tests/t.rs (target/release/deps/t-1)\ntest t_a ... FAILED\ntest t_b ... ok\n"
    assert run_integration(monkeypatch, failing) == set()


def test_a_pr_that_fixes_a_listed_test_and_deletes_its_row_is_judged_by_its_own_tree_not_the_runners(tmp_path, monkeypatch):
    """#4681 deleted the row of a test it fixes and was refused as an xpass: the list read was the runner checkout's (parity's), where the row still stood."""
    runner_list = tmp_path / "runner.toml"
    runner_list.write_text('[[xfail]]\nid = "unit crate::t_a"\nissue = 7\nowner = "o"\ndate = "2026-10-19"\n[[xfail]]\nid = "integration t::t_a"\nissue = 8\nowner = "o"\ndate = "2026-10-19"\n')
    monkeypatch.setattr(xfail, "PATH", runner_list)
    monkeypatch.setattr(xfail.load, "__defaults__", (runner_list,))
    tree = tmp_path / "tree"
    (tree / "tools").mkdir(parents=True)

    def lib(root):
        done = type("Done", (), {"stdout": "test t_a ... ok\ntest result: ok. 1 passed\n", "stderr": "", "returncode": 0})()
        return ps.dependants_lib_failures({"crate"}, root=root, run=lambda *a, **k: done)

    def integration(root):
        done = type("Done", (), {"stdout": "     Running tests/t.rs (x)\ntest t_a ... ok\n", "stderr": "", "returncode": 0})()
        return ps.integration_failures(["t"], root=root, run=lambda *a, **k: done)

    (tree / "tools" / "xfail.toml").write_text("")  # the PR deleted both rows
    assert lib(tree) == set() and integration(tree) == set()
    (tree / "tools" / "xfail.toml").write_text(runner_list.read_text())  # the PR fixed the tests and kept the rows
    assert lib(tree) == {"xpass unit crate::t_a"} and integration(tree) == {"xpass integration t::t_a"}
