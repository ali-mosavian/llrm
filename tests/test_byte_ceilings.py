"""No program of a target's corpus is over its code byte ceiling (tools/byte_ratchet.py, one case per target)."""
import subprocess
import sys
import tomllib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent


@pytest.mark.parametrize("target", tomllib.loads((ROOT / "tools/byte_ratchet.toml").read_text()))
def test_code_bytes_are_within_their_ceilings(target):
    done = subprocess.run([sys.executable, str(ROOT / "tools/byte_ratchet.py"), target], cwd=ROOT, capture_output=True, text=True)
    assert done.returncode == 0, done.stdout + done.stderr[-4000:]
