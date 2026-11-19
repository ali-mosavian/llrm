"""feat/parity is frozen: release/next is the integration branch, so no live script, workflow or test names the old one."""

from __future__ import annotations

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# History and notes (dated measurements, run logs) and the tests that must name the old branch to prove it is refused.
ALLOWED = ("docs/", "crates/", "tools/bench/waivers/", "tools/xfail.toml", "tools/budgets.toml", "tools/qcport_sweep_known.toml", "tools/test_parity_squash.py", "tools/test_landing_plan.py", "tools/ci/test_clang_trees.py", "tools/test_feat_parity_frozen.py")


def test_no_live_file_names_feat_parity():
    """Left behind after the switch: 10 tools, the workflows' comments and the docs' rules still said feat/parity, and a runner could be pointed back at it with no test failing."""
    hits = subprocess.run(["git", "grep", "-In", "feat/parity", "--", "tools", ".github", "scripts", "README.md", "Makefile"], cwd=ROOT, capture_output=True, text=True).stdout.splitlines()
    live = [line for line in hits if not line.split(":")[0].startswith(ALLOWED)]
    assert live == [], "\n".join(live)
