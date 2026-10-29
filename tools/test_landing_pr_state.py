import importlib.util
import json
import subprocess
from pathlib import Path

# Loaded by path under its own name: tools/ci/pr_state.py is also `pr_state`, and one pytest session imports both tests.
_spec = importlib.util.spec_from_file_location("landing_pr_state", Path(__file__).resolve().parent / "landing" / "pr_state.py")
pr_state = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pr_state)
import parity_squash as ps  # noqa: E402


def _graphql_reply(numbers):
    repository = {}
    for n in numbers:
        repository[f"p{n}"] = {
            "state": "OPEN", "isDraft": False, "headRefOid": f"h{n}", "headRefName": f"b{n}", "title": "t", "body": "b", "baseRefName": "feat/parity", "mergeable": "MERGEABLE",
            "commits": {"nodes": [{"commit": {"committedDate": "2026-10-27T08:00:00Z", "statusCheckRollup": {"state": "SUCCESS"}}}]},
        }
    return json.dumps({"data": {"repository": repository}})


def test_sixteen_prs_cost_one_graphql_call_not_sixteen_pr_views():
    """A cycle read each PR with its own `gh pr view` per stage (open, age, base, head): about 100 REST calls, and the quota ran out at 09:2x."""
    calls = []

    def run(cmd, **_):
        calls.append(cmd)
        numbers = [str(n) for n in range(5000, 5016)]
        return subprocess.CompletedProcess(cmd, 0, _graphql_reply(numbers), "")

    found = ps.graphql_prs([str(n) for n in range(5000, 5016)], run=run)
    assert len(calls) == 1 and calls[0][:3] == ["gh", "api", "graphql"]
    assert found["5003"]["headRefOid"] == "h5003" and found["5003"]["committedDate"] == "2026-10-27T08:00:00Z" and found["5003"]["checks"] == "SUCCESS"


def test_refresh_heads_reads_every_member_in_one_call(monkeypatch):
    """The halving batch re-read every member's head with one `gh pr view` each, per round."""
    calls = []
    monkeypatch.setattr(ps, "gh_run", lambda cmd, **_: calls.append(cmd) or subprocess.CompletedProcess(cmd, 0, _graphql_reply(["1", "2", "3"]), ""))
    infos = {"1": {"headRefOid": "h1"}, "2": {"headRefOid": "old"}, "3": {"headRefOid": "h3"}}
    assert ps.refresh_heads(infos, ["1", "2", "3"]) == {"2": ("old", "h2")} and len(calls) == 1


def test_a_filled_state_answers_until_it_is_old(monkeypatch, tmp_path):
    monkeypatch.setattr(pr_state, "TMP", tmp_path)
    monkeypatch.setattr(pr_state, "CACHE", tmp_path / "pr-state.json")
    pr_state.fill(["7"], run=lambda cmd, **_: subprocess.CompletedProcess(cmd, 0, _graphql_reply(["7"]), ""), now=lambda: 1000)
    assert pr_state.cached("7", "state", now=lambda: 1030) == "OPEN"
    assert pr_state.cached("7", "isDraft", now=lambda: 1030) == "false"
    assert pr_state.cached("7", "state", now=lambda: 1200) == "" and pr_state.cached("8", "state", now=lambda: 1030) == ""


def test_an_unchanged_parity_is_a_304_that_costs_no_quota(monkeypatch, tmp_path):
    """Each cycle polled the feat/parity ref with a full REST call; with If-None-Match an unchanged ref answers 304 and is not counted."""
    monkeypatch.setattr(pr_state, "TMP", tmp_path)
    monkeypatch.setattr(pr_state, "PARITY", tmp_path / "parity.etag")
    sent = []

    def run(cmd, **_):
        sent.append(cmd)
        if "-H" in cmd:
            return subprocess.CompletedProcess(cmd, 1, "HTTP/2.0 304 Not Modified\r\n\r\n", "gh: HTTP 304")
        return subprocess.CompletedProcess(cmd, 0, 'HTTP/2.0 200 OK\r\nEtag: W/"abc"\r\n\r\n{"object": {"sha": "s1"}}', "")

    assert pr_state.parity_sha(run) == "s1"
    assert pr_state.parity_sha(run) == "s1" and sent[1][-1] == 'If-None-Match: W/"abc"'
