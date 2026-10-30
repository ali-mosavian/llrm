"""A test skipped for want of QCport is counted; on a host that has QCport the run fails."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def run(tmp_path: Path, qcport_dir: Path):
    (tmp_path / "test_needs_qcport.py").write_text('import pytest\n\n\ndef test_it():\n    pytest.skip("QCport is not on this host")\n')
    env = {**os.environ, "PYTHONPATH": str(HERE), "LLRM_QCPORT_DIR": str(qcport_dir)}
    return subprocess.run([sys.executable, "-m", "pytest", "-p", "pytest_qcport", "-p", "no:cacheprovider", "-q", str(tmp_path / "test_needs_qcport.py")], capture_output=True, text=True, env=env, cwd=tmp_path)


def test_a_qcport_skip_where_qcport_is_absent_is_counted_and_passes(tmp_path):
    done = run(tmp_path, tmp_path / "no-such-dir")
    assert "QCPORT SKIPPED: 1" in done.stdout and done.returncode == 0, done.stdout


def test_a_qcport_skip_on_a_host_that_has_qcport_fails_the_run(tmp_path):
    """The sweep's console.c test skipped as 'QCport is not on this host' where QCport is, and the run read green."""
    (tmp_path / "qcport").mkdir()
    done = run(tmp_path, tmp_path / "qcport")
    assert "QCPORT SKIPPED: 1" in done.stdout and done.returncode != 0, done.stdout
