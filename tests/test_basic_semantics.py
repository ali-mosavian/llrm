"""Compatibility keeps BASIC's numeric helpers, not just their ordinary answers."""

from pathlib import Path
from collections import Counter
import json

import pytest

from qbopt import wholeseg
from qbopt.objectfile import module, omf
from qbopt.rewrite import Finalised, rewrite, main
from qbopt.model import mir
from qbopt.backend import lower
import corpus
from iced_x86 import Register


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("bounds_checks", [False, True])
@pytest.mark.parametrize("program", ["arridx-bounds", "ovfpol"])
def test_explicit_integer_overflow_traps_follow_numeric_policy(tag, bounds_checks, program):
    """OVFPOL's 32767+1 printed ERR 6 in native mode; it must wrap to -32768 without INTO."""
    from iced_x86 import Code, Decoder
    path = Path(f"fixtures/regressions/{program}-{tag}.obj")
    for basic in (False, True):
        result = wholeseg.emitted(path.read_bytes(), basic_semantics=basic, bounds_checks=bounds_checks)
        assert result.outcome is wholeseg.Emission.LIR, result.reason
        found = module.of(omf.parse(result.data))
        from qbopt.frontend.blocks import code_map
        mapped = code_map(found)
        assert not isinstance(mapped, str)
        # Decode at reachable instruction boundaries, not by counting CE bytes
        # inside immediates or displacements.
        traps = [at for at in mapped.starts if Decoder(16, found.code[at:]).decode().code == Code.INTO]
        assert bool(traps) is basic


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program,names", [
    ("lngmxx", {"B$DVI4", "B$RMI4"}),
    ("fpdeep", {"B$FIS2", "B$FIST"}),
])
def test_basic_keeps_exception_sensitive_helpers(tag, program, names):
    """DIVMOD's BASIC error 11 must not become #DE; FP conversions keep runtime overflow handling."""
    data = Path(f"fixtures/omf/{program}-{tag}.obj").read_bytes()
    before = module.of(omf.parse(data))
    expected = names.intersection(before.calls.values())
    assert expected
    result = wholeseg.emitted(data, basic_semantics=True)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    after = module.of(omf.parse(result.data))
    # FPDEEP's three-iteration loop is unrolled: two conversions become six
    # additional static sites, while retaining eleven dynamic conversions.
    counts = {"B$FIST": 11} if program == "fpdeep" else {"B$DVI4": 1, "B$RMI4": 1}
    assert Counter(name for name in after.calls.values() if name in expected) == counts
    native = wholeseg.emitted(data)
    assert native.outcome is wholeseg.Emission.LIR, native.reason
    assert not expected.intersection(module.of(omf.parse(native.data)).calls.values())


def test_semantic_mode_is_part_of_completion_marker():
    data = Path("fixtures/omf/lngmxx-p-g2.obj").read_bytes()
    native, _ = rewrite(data, dry_run=False)
    with pytest.raises(Finalised):
        rewrite(native, dry_run=False, basic_semantics=True)


def test_cli_records_basic_semantics_and_is_idempotent(tmp_path):
    source = Path("fixtures/omf/lngmxx-p-g2.obj")
    from qbopt.abi import linkunit

    output = tmp_path / "basic.obj"
    library = corpus.runtime_library(source)
    assert main([str(source), str(library), "-o", str(output), "--basic-semantics"]) == 0
    unit = linkunit.LinkUnit.read([source, library]).fingerprint
    assert json.loads(output.with_suffix(".json").read_text())["semantics"] == "basic"
    data = output.read_bytes()
    assert rewrite(data, dry_run=False, basic_semantics=True, contract_fingerprint=unit)[0] == data
    with pytest.raises(Finalised):
        rewrite(data, dry_run=False, contract_fingerprint=unit)


def test_retained_division_delivers_live_results_in_abi_registers():
    """LNGMXX printed 772198217 instead of 142900 when a DX return was allocated to BX."""
    path = Path("fixtures/omf/lngmxx-p-g2.obj")
    found = corpus.loaded(path)
    raised = mir.bodies(found, corpus.partitioned(path), basic_semantics=True)
    body = raised[0][1]
    hints = raised.hints[body.entry]
    op = next(op for block in body.blocks for op in block.ops
              if op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$DVI4")
    read = {value.id for block in body.blocks for one in block.ops for value in one.uses}
    lowering = lower.Lowering(body, read, found.calls, ())
    delivered = dict((held.value, register) for held, register in lowering._idiom(op))
    for value in op.defines:
        if value.id in read and not value.flags:
            assert delivered[value.id] == {Register.EAX: Register.AX, Register.EDX: Register.DX}[
                hints.origin_of(value)]
