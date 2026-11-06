"""A test that skips on "<bin> is not built" fails the gate's pytest run: llrm-c was not built for weeks and the readme's C case skipped."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def run(tmp_path: Path, reason: str, strict: bool):
    (tmp_path / "test_x.py").write_text(f'import pytest\n\n\ndef test_it():\n    pytest.skip("{reason}")\n')
    plugins = ["-p", "pytest_strict_bins"] * strict
    return subprocess.run([sys.executable, "-m", "pytest", *plugins, "-p", "no:cacheprovider", "-q", str(tmp_path / "test_x.py")], capture_output=True, text=True, env={**os.environ, "PYTHONPATH": str(HERE)})


def test_a_skip_for_a_binary_that_is_not_built_fails_the_run(tmp_path):
    done = run(tmp_path, "llrm-c is not built (debt skip-readme-c)", strict=True)
    assert done.returncode != 0 and "skipped for a binary the gate builds" in done.stdout, done.stdout


def test_the_same_skip_without_the_plugin_passes_which_is_how_it_hid(tmp_path):
    assert run(tmp_path, "llrm-c is not built", strict=False).returncode == 0


def test_a_skip_for_a_host_capability_is_left_alone(tmp_path):
    assert run(tmp_path, "needs the pin's llc", strict=True).returncode == 0
