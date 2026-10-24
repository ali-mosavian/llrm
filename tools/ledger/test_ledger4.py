import os
import subprocess
import sys
from pathlib import Path

TOOL = Path(__file__).with_name("ledger4.py")


def run(cwd, *args, **env):
    return subprocess.run(
        [sys.executable, str(TOOL), *args],
        cwd=cwd,
        env={"PATH": os.environ["PATH"], **env},
        capture_output=True,
        text=True,
    )


def test_a_stale_default_file_is_not_read_when_the_inputs_are_not_named(tmp_path):
    """w-2 passed file names as arguments; the script ignored them and read
    `ours.incl` and `../ledger2/clang.incl` from wherever it ran, with the totals
    of another build, and printed a table."""
    (tmp_path / "ours.incl").write_text("     100 (100.0%)  PROGRAM TOTALS\n")
    done = run(tmp_path)
    assert done.returncode != 0
    assert "LEDGER_OURS, LEDGER_CLANG, LEDGER_OT, LEDGER_PT not set" in done.stderr
    assert "Traceback" not in done.stderr


def test_positional_arguments_are_refused_not_ignored(tmp_path):
    done = run(tmp_path, "ours.incl", "clang.incl", LEDGER_OURS="a", LEDGER_CLANG="b", LEDGER_OT="1", LEDGER_PT="1")
    assert done.returncode != 0
    assert "positional arguments are not supported" in done.stderr


def test_one_missing_variable_is_named(tmp_path):
    done = run(tmp_path, LEDGER_OURS="a", LEDGER_CLANG="b", LEDGER_OT="1")
    assert done.returncode != 0 and "LEDGER_PT not set" in done.stderr
