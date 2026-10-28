"""The QCport tier of the oracle sweep: a module that panics under an oracle is a finding, and a run that drew nothing is no pass."""
from __future__ import annotations

import stat
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import oracle_qcport  # noqa: E402
import oracle_sweep  # noqa: E402

PANIC = "thread 'main' panicked at crates/opt/llrm-analysis/src/memory.rs:10:5:\nunsound memory summary\n"


def compiler(tmp_path: Path, body: str) -> Path:
    path = tmp_path / "llrm-c"
    path.write_text("#!/bin/sh\n" + body)
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return path


def test_a_module_that_panics_under_an_oracle_is_a_finding_with_its_message(tmp_path):
    """The C corpus the sweep lacked: an unsound memory summary reached main and QCport's sc was the only thing that ran it."""
    fake = compiler(tmp_path, f"printf '%s' '{PANIC}' >&2; exit 101\n")
    why, _ = oracle_qcport.compile_module(fake, tmp_path / "sc.c", "-O2", [], tmp_path / "sc.obj")
    assert why == "unsound memory summary"


def test_a_module_that_compiles_reports_the_oracles_its_compile_reached(tmp_path):
    fake = compiler(tmp_path, "echo '[checks] DATAFLOW 3' >&2; echo '[checks] FACTS 2' >&2\n")
    why, reached = oracle_qcport.compile_module(fake, tmp_path / "sc.c", "-O2", [], tmp_path / "sc.obj")
    assert why is None and reached == {"DATAFLOW": 3, "FACTS": 2}


def test_every_oracle_is_on_in_the_compile_environment():
    env = oracle_qcport.oracle_env({"LLRM_DEBUG": "time"})
    assert all(env[f"LLRM_CHECK_{name}"] == "1" for name in oracle_sweep.CHECKS)
    assert env["LLRM_DEBUG"] == "time,checks"


def test_a_run_that_drew_nothing_is_no_pass():
    """qcport-run-cc printed its summary only for a draw; no line, or zero polygons, is not one."""
    good = "QCport: 65 modules at -Os draw what the Borland build draws: 300 frames, 12000 polys, bmp abcd1234"
    assert oracle_qcport.drew(good) == (300, 12000)
    assert oracle_qcport.drew(good.replace("12000 polys", "0 polys")) is None
    assert oracle_qcport.drew("a run did not finish: llrm") is None
