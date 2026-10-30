"""qcport.py refuses a report in which a compile failed: both arms failing everything is a broken instrument, not a result."""
import os
import stat
import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "qcport.py"


def compiler(path: Path, works: bool) -> Path:
    path.write_text("#!/bin/sh\n" + "[ \"$1\" = --version ] && { echo 'llrm-c abc123'; exit 0; }\n" + ("echo x > \"$(echo \"$@\" | sed 's/.*-o \\([^ ]*\\) .*/\\1/')\"\n" if works else "exit 1\n"))
    path.chmod(path.stat().st_mode | stat.S_IEXEC)
    return path


def test_a_report_with_failed_compiles_is_void(tmp_path):
    """w-1's ex4: the build tree the dist reads its C headers from was removed, every compile failed on both arms (616 programs), and the report went on."""
    src = tmp_path / "src"
    for module in ("host", "render", "model", "game", "sound", "ui"):
        (src / module).mkdir(parents=True)
        (src / module / "a.c").write_text("int a;\n")
    env = {**os.environ, "QCPORT_SRC": str(src), "QCPORT_INC": str(tmp_path), "ROWS_WORK": str(tmp_path), "LLRM_ALLOW_DIRTY": "1",
           "BASE_LLRM": str(compiler(tmp_path / "base", False)), "MAIN_LLRM": str(compiler(tmp_path / "main", False))}
    done = subprocess.run([sys.executable, str(SCRIPT)], env=env, capture_output=True, text=True)
    assert done.returncode != 0 and "the report is void" in done.stderr + done.stdout, done
