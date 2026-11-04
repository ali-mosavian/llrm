"""nibprobe's matrix lines: one per conf and subject, each saying the death-screen pixels and the emulated time and cycles it came at."""
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import nibprobe  # noqa: E402
import qb32  # noqa: E402
import qb32demos  # noqa: E402
import qbbuild  # noqa: E402
import qbplay  # noqa: E402
import qbruntime  # noqa: E402


@pytest.mark.parametrize("label,edit,timed", nibprobe.VARIANTS)
def test_a_variant_changes_what_it_names(label, edit, timed):
    """A conf edit that matches nothing (the conf's text moved) would print a second base row under another name."""
    assert nibprobe.edited(label, edit, qb32.DEMO_CONF) != qb32.DEMO_CONF or timed or label == "base"
    assert (label == "keys=timed") == timed


def test_every_conf_and_subject_prints_its_pixels_time_and_cycles(monkeypatch, capsys, tmp_path):
    played = []

    def play(exe, work, steps, demo=None, conf="", timed=False, ended=None):
        played.append((conf, timed))
        ended.update(us=8_000_000, cycles_per_ms=2000)
        shot = work / "dies.png"
        shot.write_bytes(b"")
        return {"dies": shot}

    monkeypatch.setattr(qbruntime, "demo_sources", lambda: ({"x": tmp_path / "NIBBLES.BAS"}, ""))
    monkeypatch.setattr(qbbuild, "runtimes", lambda work: {})
    monkeypatch.setattr(qbbuild, "build", lambda *a, **k: {qbbuild.REAL: SimpleNamespace(exe=tmp_path / "n.exe", files=())})
    monkeypatch.setattr(qbplay, "reference", lambda sources, work: {"NIBBLES": tmp_path / "r.exe"})
    monkeypatch.setattr(qb32demos, "reference_shots", lambda *a: {"dies": tmp_path / "want.png"})
    monkeypatch.setattr(qb32demos, "play", play)
    monkeypatch.setattr(qbplay, "differing_pixels", lambda want, got: 38)
    assert nibprobe.main() == 0
    lines = [line for line in capsys.readouterr().out.splitlines() if line.startswith("nibprobe ")]
    assert len(lines) == 2 * len(nibprobe.VARIANTS)
    assert all("38 pixels at dies, 8000000 emulated us, 16000000 cycles" in line for line in lines)
    assert sum(timed for _, timed in played) == 2 and len({conf for conf, _ in played}) == len(nibprobe.VARIANTS) - 1
