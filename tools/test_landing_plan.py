"""`queue-runner.sh --print-plan`: what the runner would do, read-only, so the switch to release/next is shown and not only grepped."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

RUNNER = Path(__file__).parent / "landing" / "queue-runner.sh"


def plan(tmp_path: Path, prs: list[dict]) -> str:
    listing = tmp_path / "prs.json"
    listing.write_text(json.dumps(prs))
    env = {k: v for k, v in os.environ.items() if k != "LAND_BRANCH"}
    done = subprocess.run(["bash", str(RUNNER), "--print-plan", "--prs", str(listing)], env={**env, "LAND_TMP": str(tmp_path / "land")}, capture_output=True, text=True, check=True)
    assert not (tmp_path / "land").exists(), "a plan writes nothing: no pid file, no queue"
    return done.stdout


def test_the_plan_lands_on_release_next_refuses_the_old_base_and_names_the_old_branch_nowhere(tmp_path):
    """The switch was only grepped: nothing showed the runner landing on release/next or turning a PR on the frozen branch away."""
    ready = {"isDraft": False, "labels": [{"name": "ready"}]}
    out = plan(tmp_path, [{"number": 6001, "baseRefName": "feat/parity", **ready}, {"number": 6002, "baseRefName": "release/next", **ready}, {"number": 6003, "baseRefName": "release/next", "isDraft": True, "labels": []}])
    assert "land onto release/next" in out and "fetch origin release/next" in out
    assert "6001: REFUSED: base is not release/next" in out
    assert "6002: LAND onto release/next, batch position 1" in out
    assert "6003" not in out, "a draft is not queued"
    assert "feat/parity" not in out, out
