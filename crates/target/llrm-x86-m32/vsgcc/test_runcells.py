import os
import subprocess
import tempfile
from pathlib import Path

RUNCELLS = os.environ.get("RUNCELLS_SH", str(Path(__file__).with_name("runcells.sh")))


def test_a_program_that_cannot_be_built_fails_the_run_and_names_its_cell():
    """The 2026-11-09 matrix had 9 blank cells: a parallel driver swallowed each failed variant and exited 0, so the table read as complete."""
    with tempfile.TemporaryDirectory() as work:
        env = dict(os.environ, VSGCC_WORK=work)
        run = subprocess.run(["bash", RUNCELLS, f"{work}/out.jsonl", "no_such_program_xyz"], capture_output=True, text=True, env=env)
        assert run.returncode != 0
        assert "FAILED no_such_program_xyz" in run.stdout


def test_a_call_without_programs_is_a_usage_error():
    run = subprocess.run(["bash", RUNCELLS, "/dev/null"], capture_output=True, text=True)
    assert run.returncode == 2
