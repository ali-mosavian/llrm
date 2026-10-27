"""One launch for all 848 programs ended at the first flooding program and reported the 262 after it as failed: a group of its own contains the flood."""
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))
import run_groups  # noqa: E402


def fake(outputs):
    def run(command, **kw):
        group = command[-1]
        return SimpleNamespace(stdout=outputs[group], stderr="", returncode=0)
    return run


def test_a_flooded_group_is_named_and_does_not_hide_the_other_groups(monkeypatch, capsys):
    out = {
        "c": "2 programs: 2 pass, 0 known, 0 fail\n",
        "examples": "FAIL  examples/a.nib: stopped: the launch was stopped when events.txt passed 33554432 bytes\n3 programs: 0 pass, 0 known, 3 fail\n",
        "nib": "FAIL  nib/x.nib: line 1: want '1', got '0'\n1 programs: 0 pass, 0 known, 1 fail\n",
    }
    monkeypatch.setattr(subprocess, "run", fake(out))
    monkeypatch.setattr(sys, "argv", ["run_groups.py", "c", "examples", "nib"])
    assert run_groups.main() == 1
    text = capsys.readouterr().out
    assert "[c] 2 programs: 2 pass" in text and "LAUNCH STOPPED" in text and "[nib] 1 programs" in text and "nib/x.nib" in text


def test_a_clean_run_exits_zero(monkeypatch, capsys):
    monkeypatch.setattr(subprocess, "run", fake({"c": "5 programs: 5 pass, 0 known, 0 fail\n"}))
    monkeypatch.setattr(sys, "argv", ["run_groups.py", "c"])
    assert run_groups.main() == 0


def test_the_isel_proof_line_of_each_group_is_printed(monkeypatch, capsys):
    """A DAG run showed no proof line, so nothing said the DAG path had selected the functions."""
    out = {"c": "isel proof (x.txt): LLRM_ISEL=dag: 12 functions selected by the DAG path, 0 by the old selector\n5 programs: 5 pass, 0 known, 0 fail\n"}
    monkeypatch.setattr(subprocess, "run", fake(out))
    monkeypatch.setattr(sys, "argv", ["run_groups.py", "c"])
    run_groups.main()
    assert "12 functions selected by the DAG path" in capsys.readouterr().out
