"""`--help` of every argparse tool in tools/ exits 0: a bare `%` in a help string crashed compile-gap.py's on main."""

import subprocess
import sys
from pathlib import Path

import pytest

TOOLS = Path(__file__).parent
SCRIPTS = sorted(one for one in TOOLS.glob("*.py") if not one.name.startswith("test_") and "argparse.ArgumentParser(" in one.read_text() and "__main__" in one.read_text())


@pytest.mark.parametrize("script", SCRIPTS, ids=lambda one: one.name)
def test_help_exits_zero(script):
    done = subprocess.run([sys.executable, str(script), "--help"], capture_output=True, text=True, timeout=60, cwd=TOOLS.parent, stdin=subprocess.DEVNULL)
    assert done.returncode == 0, done.stderr.strip().splitlines()[-1:]
