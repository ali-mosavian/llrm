"""row.sh exits nonzero when the build or the harness crashes: a crash used to leave rc 0 and the row missing from the table."""
import os
import shutil
import subprocess
from pathlib import Path

HERE = Path(__file__).parent


def tree(tmp_path, build, harness):
    root = tmp_path / "t"
    (root / "tools" / "rows").mkdir(parents=True)
    shutil.copy(HERE / "row.sh", root / "tools" / "rows" / "row.sh")
    (root / "tools" / "stamp.py").write_text("")
    (root / "tools" / "pyproject.toml").write_text('[project]\nname = "stub"\nversion = "0"\nrequires-python = ">=3.8"\n')
    vs = root / "crates" / "target" / "x" / "vsgcc"
    vs.mkdir(parents=True)
    (vs / "run.sh").write_text("")
    (vs / "build.sh").write_text(build)
    (vs / "build.sh").chmod(0o755)
    (vs / "harness.py").write_text(harness)
    work = tmp_path / "w"
    (work / "main24d" / "b").mkdir(parents=True)
    (work / "main24d" / "abi").write_text("sysv")
    return root, work


def run(root, work):
    env = dict(os.environ, ROWS_WORK=str(work), TMPDIR=str(work))
    return subprocess.run(["bash", str(root / "tools" / "rows" / "row.sh"), "p", "llrm", "/bin/true"], env=env, capture_output=True, text=True)


def test_a_harness_crash_is_a_failed_row(tmp_path):
    root, work = tree(tmp_path, "#!/bin/bash\nexit 0\n", "import sys\nprint('boom')\nsys.exit(1)\n")
    assert run(root, work).returncode != 0


def test_a_build_crash_without_a_fail_line_is_a_failed_row(tmp_path):
    root, work = tree(tmp_path, "#!/bin/bash\necho cannot start >&2\nexit 7\n", "print('{\"clocks\": 1}')\n")
    assert run(root, work).returncode != 0


def test_a_good_build_and_harness_print_the_row(tmp_path):
    root, work = tree(tmp_path, "#!/bin/bash\nexit 0\n", "print('{\"clocks\": 1}')\n")
    got = run(root, work)
    assert got.returncode == 0 and got.stdout.strip() == '{"clocks": 1}'
