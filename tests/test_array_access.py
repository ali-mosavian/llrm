"""Real /D ARRIDX output: checking and address calculation are one helper."""

from pathlib import Path
import json

import pytest
import corpus

from qbopt import wholeseg
from qbopt.model import mir
from qbopt.objectfile import module, omf
from qbopt.rewrite import Finalised, main, rewrite


def test_overflow_observation_has_no_normal_path_register_results():
    """/D ARRIDX printed 630 instead of 1260: INTO invented a new AX result allocated to BX."""
    path = Path("fixtures/regressions/arridx-bounds-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path), bounds_checks=True)[0][1]
    checks = [op for block in body.blocks for op in block.ops if found.code[op.at:op.at + 1] == b"\xce"]
    assert checks
    assert all(not op.defines for op in checks)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("basic_semantics", [False, True])
@pytest.mark.parametrize("bounds_checks", [False, True])
def test_array_checks_are_independent_of_numeric_semantics(tag, basic_semantics, bounds_checks):
    """ARRIDX's three HARY calls must become address arithmetic, not deleted pointer definitions."""
    path = Path(f"fixtures/regressions/arridx-bounds-{tag}.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path), basic_semantics=basic_semantics,
                      bounds_checks=bounds_checks)[0][1]
    calls = [op for block in body.blocks for op in block.ops
             if op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$HARY"]
    assert len(calls) == (3 if bounds_checks else 0)
    if not bounds_checks:
        assert sum(op.kind is mir.Kind.MUL for block in body.blocks for op in block.ops) >= 3
    output = wholeseg.emitted(path.read_bytes(), basic_semantics=basic_semantics, bounds_checks=bounds_checks)
    assert output.outcome is wholeseg.Emission.LIR, output.reason
    after = module.of(omf.parse(output.data))
    assert list(after.calls.values()).count("B$HARY") == (3 if bounds_checks else 0)
    assert list(after.calls.values()).count("B$LINA") == list(found.calls.values()).count("B$LINA")


def test_bounds_policy_is_recorded_separately(tmp_path):
    """Numeric compatibility must not silently enable array checks or reuse unchecked output."""
    source = Path("fixtures/regressions/arridx-bounds-p-g2.obj")
    output = tmp_path / "checked.obj"
    assert main([str(source), "-o", str(output), "--bounds-checks"]) == 0
    report = json.loads(output.with_suffix(".json").read_text())
    assert report["bounds_checks"] is True
    assert report["semantics"] == "native"
    data = output.read_bytes()
    assert rewrite(data, dry_run=False, bounds_checks=True)[0] == data
    with pytest.raises(Finalised):
        rewrite(data, dry_run=False)


def test_unsupported_checked_helper_is_not_unchecked_success():
    """An unrecognized descriptor must not ship HARY checks as successful unchecked lowering."""
    from unittest.mock import patch
    path = Path("fixtures/regressions/arridx-bounds-p-g2.obj")
    with patch("qbopt.frontend.raising_array_access.descriptor", return_value=None):
        result = wholeseg.emitted(path.read_bytes())
        with pytest.raises(ValueError, match="unchecked array"):
            rewrite(path.read_bytes(), dry_run=False)
    assert result.outcome is wholeseg.Emission.REFUSED
    assert "unchecked array" in result.reason
