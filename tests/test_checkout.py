"""A script run from one checkout silently imported another's qbopt through the shared venv."""

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_importing_another_checkouts_qbopt_is_refused(tmp_path: Path) -> None:
    """qbstages, select_sweep and wsdump each compared against the main checkout's compiler."""
    (tmp_path / "qbopt").mkdir()
    (tmp_path / "qbopt" / "__init__.py").write_text("")
    script = tmp_path / "tools" / "where.py"
    script.parent.mkdir()
    script.write_text("import qbopt\n")
    env = {**os.environ, "PYTHONPATH": str(ROOT)}
    ran = subprocess.run([sys.executable, str(script)], cwd=tmp_path, env=env, capture_output=True, text=True)
    assert ran.returncode and "another checkout" in ran.stderr


def test_this_checkouts_qbopt_imports_from_a_subfolder() -> None:
    ran = subprocess.run(
        [sys.executable, "-c", "import qbopt"], cwd=ROOT / "tools", env={**os.environ, "PYTHONPATH": str(ROOT)}
    )
    assert not ran.returncode
