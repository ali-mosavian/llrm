import sys
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import abmeasure  # noqa: E402


def test_the_build_is_the_gates_not_the_local_default():
    """An instruction delta measured with `incremental = true` (the release profile's) had the opposite sign of CI's: +0.24% against -0.58% (#2077)."""
    env = abmeasure.build_env(Path("/t"))
    assert env["CARGO_INCREMENTAL"] == "0" and "RUSTFLAGS" not in env and env["CARGO_TARGET_DIR"] == "/t"
    assert "CARGO_INCREMENTAL=0 env -u RUSTFLAGS cargo build --profile dist --bin llrm-c" in abmeasure.__doc__


def test_perf_counts_are_read_by_event_name(monkeypatch):
    done = subprocess.CompletedProcess([], 0, stdout="", stderr="6016825,,instructions:u,2367800,100.00,1.26,insn per cycle\n4777227,,cycles:u,2367800,100.00,,\n")
    monkeypatch.setattr(abmeasure.subprocess, "run", lambda *a, **k: done)
    assert abmeasure.counted(Path("c"), Path("s"), [], "O2", "cycles:u,instructions:u") == {"instructions": 6016825, "cycles": 4777227}


def test_callgrinds_cache_lines_are_read(monkeypatch):
    text = """==1== I   refs:      106,272,688
==1== D1  misses:        1,032,069  (   686,020 rd +    346,049 wr)
==1== LL misses:           134,784  (    91,031 rd +     43,753 wr)
"""
    monkeypatch.setattr(abmeasure.subprocess, "run", lambda *a, **k: subprocess.CompletedProcess([], 0, stdout="", stderr=text))
    assert abmeasure.callgrind(Path("c"), Path("s"), [], "O2") == {"Ir": 106272688, "D1mr": 686020, "D1mw": 346049, "LL": 134784}


def test_a_change_is_signed_and_in_percent():
    assert abmeasure.change(100, 99.5) == "-0.50%" and abmeasure.change(2162.25, 2167.44) == "+0.24%"


def test_d_faces_is_left_out_without_qcport(tmp_path, capsys):
    got = abmeasure.programs(tmp_path, ["d_faces", "lru"], None, tmp_path)
    assert list(got) == ["lru"] and "QCPORT is not set" in capsys.readouterr().err
