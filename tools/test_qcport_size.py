import importlib.util
import subprocess
import sys
from pathlib import Path

TOOL = Path(__file__).resolve().parent / "qcport-size.py"


def test_an_unset_environment_is_one_clear_line_not_a_traceback():
    done = subprocess.run([sys.executable, str(TOOL)], capture_output=True, text=True, env={"PATH": "/usr/bin:/bin"})
    assert done.returncode != 0
    assert "QCPORT is not set" in done.stderr and "Traceback" not in done.stderr


def test_the_default_row_is_the_register_convention_llrm_c_has():
    spec = importlib.util.spec_from_file_location("qcport_size", TOOL)
    source = TOOL.read_text()
    assert spec and 'default="x86_16-dos-rp3"' in source
