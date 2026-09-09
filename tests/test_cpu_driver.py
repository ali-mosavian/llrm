"""A tuning target must reach emission and survive the finalisation marker."""
import json
from pathlib import Path

import pytest
from iced_x86 import Mnemonic
import corpus
from qbopt import rewrite

FIXTURE = Path("fixtures/omf/lngmxx-p-g2.obj")


def test_cli_cpu_selects_reciprocal_and_records_target(tmp_path):
    output = tmp_path / "lngmxx.obj"
    assert rewrite.main([str(FIXTURE), "-o", str(output), "--cpu", "P5"]) == 0
    data = output.read_bytes()
    assert not any(one.insn.mnemonic == Mnemonic.IDIV for block in corpus.partitioned(data) for one in block.insns)
    assert json.loads(output.with_suffix(".json").read_text())["cpu"] == "P5"
    assert rewrite.rewrite(data, dry_run=False, cpu="P5")[0] == data
    with pytest.raises(rewrite.Finalised):
        rewrite.rewrite(data, dry_run=False, cpu="386")


def test_default_marker_means_386_not_unspecified():
    data = rewrite.rewrite(FIXTURE.read_bytes(), dry_run=False)[0]
    assert rewrite.rewrite(data, dry_run=False, cpu="386")[0] == data
    with pytest.raises(rewrite.Finalised):
        rewrite.rewrite(data, dry_run=False, cpu="P5")


def test_e2e_cli_passes_cpu_to_rewrite(monkeypatch):
    import e2e
    from types import SimpleNamespace
    observed = []
    def run(*args, transform, **kwargs):
        observed.append(transform(FIXTURE.read_bytes()))
        return SimpleNamespace(verdicts=[], ok=True)
    monkeypatch.setattr(e2e, "run", run)
    assert e2e.main(["p-g2", "--prog", "lngmxx", "--cpu", "P5"]) == 0
    assert observed == [rewrite.rewrite(FIXTURE.read_bytes(), dry_run=False, cpu="P5")[0]]
