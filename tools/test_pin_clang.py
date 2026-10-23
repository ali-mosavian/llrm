import subprocess
import sys
from pathlib import Path

import pytest

TOOL = Path(__file__).with_name("pin_clang.py")


def run(env_path):
    return subprocess.run(
        [sys.executable, str(TOOL)],
        env={"LLRM_PIN_CLANG": env_path, "PATH": "/usr/bin:/bin"},
        capture_output=True,
        text=True,
    )


@pytest.mark.skipif(not Path("/usr/bin/clang").exists(), reason="no distro clang")
def test_the_distro_clang_is_refused_as_the_pin():
    """The vsgcc scripts and a bare `clang` ledger line ran PATH clang, the
    distro 20.1.8, whose pass pipeline is not the pin's: its Ir read as the pin."""
    done = run("/usr/bin/clang")
    assert done.returncode != 0
    assert "d1106deb71cc" in done.stderr


def test_a_missing_binary_is_refused():
    assert run("/nonexistent/clang").returncode != 0
