"""The hosted runner has no buildslot, so the setup action tells tools/buildslot.py to leave cargo as it is there.

Gate run 2 (python group): ten test files that pass alone failed on the hosted runner with 'buildslot is not installed (~/.local/bin/buildslot):
refusing to run cargo outside a build slot' (tools/buildslot.py). tinytim keeps the slot rule."""

import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SETUP = (ROOT / ".github/actions/setup/action.yml").read_text()
STEP = "name: No build slot on a hosted runner"


def step() -> str:
    return SETUP.split(STEP)[1].split("\n    - ")[0]


def test_the_step_runs_on_hosted_runners_only():
    assert "if: runner.environment == 'github-hosted'" in step()


def test_the_step_exports_the_bypass(tmp_path):
    env_file = tmp_path / "env"
    env_file.write_text("")
    script = step().split("run: |\n")[1]
    subprocess.run(["bash", "-c", script], check=True, env={**os.environ, "GITHUB_ENV": str(env_file)})
    assert env_file.read_text().split() == ["BUILDSLOT_BYPASS=1"]


def test_buildslot_still_refuses_without_the_bypass(monkeypatch, tmp_path):
    """The bypass is the runner's, not the code's: with it cleared and no buildslot installed, cargo is refused."""
    import sys

    sys.path.insert(0, str(ROOT / "tools"))
    import buildslot

    monkeypatch.delenv("BUILDSLOT_BYPASS", raising=False)
    monkeypatch.delenv(buildslot.LANDING, raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))
    monkeypatch.setenv("HOME", str(tmp_path))
    try:
        buildslot.slotted("cargo build")
        raise AssertionError("cargo ran outside a slot")
    except SystemExit as stop:
        assert "refusing to run cargo outside a build slot" in str(stop)
