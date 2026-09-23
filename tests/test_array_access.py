"""Real /D ARRIDX output: checking and address calculation are one helper."""

from pathlib import Path
import json

import pytest
import corpus

from qbopt import wholeseg
from qbopt.model import mir
from qbopt.objectfile import module, omf
from qbopt.rewrite import Finalised, main, rewrite


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("zero_only", [False, True])
def test_checked_constant_indices_can_use_native_addressing(tag, zero_only, monkeypatch):
    """NDMAX's zero-index access retained sixty checks; folding its offset also refused emission."""
    if zero_only:
        from qbopt.frontend import raising_array_access
        checked = raising_array_access._checked
        monkeypatch.setattr(raising_array_access, "_checked",
                            lambda shape, symbol, indices, memory:
                            checked(shape, symbol, indices, memory) and all(index.n == 0 for index in indices))
    path = Path(f"fixtures/regressions/ndmax-{tag}.obj".lower())
    found = corpus.loaded(path)
    first = min(at for at, name in found.calls.items() if name == "B$HARY")
    body = mir.bodies(found, corpus.partitioned(path), bounds_checks=True)[0][1]
    assert not any(op.at == first and op.kind is mir.Kind.CALL
                   for block in body.blocks for op in block.ops)
    emitted = wholeseg.emitted(path.read_bytes(), bounds_checks=True)
    assert emitted.outcome is wholeseg.Emission.LIR, emitted.reason
    assert list(module.of(omf.parse(emitted.data)).calls.values()).count("B$HARY") < list(found.calls.values()).count("B$HARY")


@pytest.mark.parametrize("hazard", ["none", "below", "above", "unknown-index", "rank", "features", "width", "unknown-bound"])
def test_checked_proof_requires_each_live_dimension(hazard):
    """An out-of-range dimension can flatten into a valid allocation offset; that is still a bounds error."""
    from qbopt.analysis import consts
    from qbopt.frontend import raising_array_access
    from qbopt.objectfile.module import Addr, Space
    symbol = mir.Symbol(Space.SEGMENT, 5, 0, 2)
    field = lambda offset, width=2: mir.MemRef(Addr(Space.SEGMENT, offset, 5), width)
    shape = raising_array_access.Descriptor(field(0), field(2), 2,
                                           ((field(14), field(16)), (field(18), field(20))))
    memory = {}
    for offset, width, number in ((8, 1, 2), (9, 1, 1), (12, 2, 2), (14, 2, 2), (16, 2, 0xffff), (18, 2, 3), (20, 2, 2)):
        if hazard == "unknown-bound" and offset == 18:
            continue
        if offset == {"rank": 8, "features": 9, "width": 12}.get(hazard):
            number = 0
        memory.update(consts._fragments(field(offset, width), consts.Known(number, width)))
    indices = [consts.Known(3, 2), consts.Known(0xffff, 2)]
    if hazard == "below":
        indices[1] = consts.Known(0xfffe, 2)
    if hazard == "above":
        indices[1] = consts.Known(1, 2)
    if hazard == "unknown-index":
        indices[1] = None
    assert raising_array_access._checked(shape, symbol, indices, memory) == (hazard == "none")


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_checked_access_proofs_reach_fixed_point(tag):
    """NDMAX retained three HARY checks after its first proven store; two also have constant valid indices."""
    path = Path(f"fixtures/regressions/ndmax-{tag}.obj".lower())
    found = corpus.loaded(path)
    sites = sorted(at for at, name in found.calls.items() if name == "B$HARY")
    body = mir.bodies(found, corpus.partitioned(path), bounds_checks=True)[0][1]
    retained = [op.at for block in body.blocks for op in block.ops
                if op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$HARY"]
    assert retained == sites[-1:]
    emitted = wholeseg.emitted(path.read_bytes(), bounds_checks=True)
    assert emitted.outcome is wholeseg.Emission.LIR, emitted.reason
    assert list(module.of(omf.parse(emitted.data)).calls.values()).count("B$HARY") == 1


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program", ["ndarr", "ndmax"])
def test_hary_supports_nine_and_sixty_dimensions(tag, program):
    """NDARR (1,12,2) and NDMAX (11,22) were refused by an invented eight-dimension cap."""
    path = Path(f"fixtures/regressions/{program}-{tag}.obj".lower())
    found = corpus.loaded(path)
    assert "B$HARY" in found.calls.values()
    raised = mir.bodies(found, corpus.partitioned(path))
    body = raised[0][1]
    assert not any(op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$HARY"
                   for block in body.blocks for op in block.ops)
    result = wholeseg.emitted(path.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert "B$HARY" not in module.of(omf.parse(result.data)).calls.values()


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_native_array_arithmetic_keeps_allocation_dimension_constants(tag):
    """NDMAX grew to 9.7 KB because each native op was mistaken for the removed HARY call."""
    from qbopt.analysis import consts
    path = Path(f"fixtures/regressions/ndmax-{tag}.obj".lower())
    found = corpus.loaded(path)
    raised = mir.bodies(found, corpus.partitioned(path))
    body = raised[0][1]
    facts = consts.known(body, found.dgroup, found.calls)
    initialized = {ref: value for block in body.blocks for op in block.ops for ref, value in op.memory_values}
    first = min(at for at, name in found.calls.items() if name == "B$HARY")
    loads = [op for block in body.blocks for op in block.ops
             if op.at == first and op.kind is mir.Kind.LOAD and op.loads[0] in initialized]
    assert len(loads) >= 60
    for op in loads:
        expected = initialized[op.loads[0]]
        assert facts.get(op.results[0].value) == consts.Known(expected.n, expected.width)


def test_overflow_observation_has_no_normal_path_register_results():
    """/D ARRIDX printed 630 instead of 1260: INTO invented a new AX result allocated to BX."""
    path = Path("fixtures/regressions/arridx-bounds-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path), basic_semantics=True, bounds_checks=True)[0][1]
    checks = [op for block in body.blocks for op in block.ops if found.code[op.at:op.at + 1] == b"\xce"]
    assert checks
    # An interrupt may define a selector; nothing on the normal path may read it.
    invented = {value for op in checks for value in op.defines}
    merged = True
    while merged:
        merged = {phi.result for block in body.blocks for phi in block.phis
                  if invented & set(phi.incoming.values())} - invented
        invented |= merged
    assert not [op for block in body.blocks for op in block.ops if invented & set(op.uses)]


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("basic_semantics", [False, True])
@pytest.mark.parametrize("bounds_checks", [False, True])
def test_array_checks_are_independent_of_numeric_semantics(tag, basic_semantics, bounds_checks):
    """ARRIDX's three HARY calls must become address arithmetic, not deleted pointer definitions."""
    path = Path(f"fixtures/regressions/arridx-bounds-{tag}.obj".lower())
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
    from qbopt.abi import linkunit

    output = tmp_path / "checked.obj"
    library = corpus.runtime_library(source)
    assert main([str(source), str(library), "-o", str(output), "--bounds-checks"]) == 0
    unit = linkunit.LinkUnit.read([source, library]).fingerprint
    report = json.loads(output.with_suffix(".json").read_text())
    assert report["bounds_checks"] is True
    assert report["semantics"] == "native"
    data = output.read_bytes()
    assert rewrite(data, dry_run=False, bounds_checks=True, contract_fingerprint=unit)[0] == data
    with pytest.raises(Finalised):
        rewrite(data, dry_run=False, contract_fingerprint=unit)


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


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program", ["harr-bounds", "dynsz"])
def test_dynamic_far_address_arithmetic_is_native(tag, program):
    """HARR computes both addresses via HARY each iteration; they must be MIR, not retained calls."""
    path = Path(f"fixtures/regressions/{program}-{tag}.obj".lower())
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    assert not any(op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$HARY"
                   for block in body.blocks for op in block.ops)
    assert any(op.kind is mir.Kind.MUL for block in body.blocks for op in block.ops)
    result = wholeseg.emitted(path.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert "B$HARY" not in module.of(omf.parse(result.data)).calls.values()
    if program == "dynsz":
        assert not any(op.array for block in body.blocks for op in block.ops)


def test_dynamic_shape_does_not_freeze_descriptor_fields():
    """HARR's DIM facts do not make heap addresses or bounds immutable across later calls."""
    from qbopt.frontend import raising_array_access
    path = Path("fixtures/regressions/harr-bounds-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path), bounds_checks=True)[0][1]
    request = next(op.array for block in body.blocks for op in block.ops if op.array)
    shape = raising_array_access.dynamic(body, request.descriptor)
    assert isinstance(shape.data, mir.MemRef)
    assert all(isinstance(field, mir.MemRef) for dimension in shape.dimensions for field in dimension)


@pytest.mark.parametrize("features", [0, 2, 3, 0x81])
def test_dynamic_unestablished_layout_is_not_assumed_far(features):
    from dataclasses import replace
    from qbopt.frontend import raising_array_access
    path = Path("fixtures/regressions/harr-bounds-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path), bounds_checks=True)[0][1]
    allocation = next(op for block in body.blocks for op in block.ops if op.array)
    request = allocation.array
    fields = tuple((ref, mir.Const(features, 1) if ref.addr.disp == request.descriptor.offset + 9 else value)
                   for ref, value in allocation.memory_values)
    body = replace(body, blocks=tuple(replace(block, ops=tuple(
        replace(op, memory_values=fields) if op is allocation else op for op in block.ops))
        for block in body.blocks))
    shape = raising_array_access.dynamic(body, request.descriptor)
    if features in (2, 3):
        assert shape.huge and shape.data.width == 4
    else:
        assert shape is None


def test_dynamic_address_uses_runtime_lower_bounds_and_correct_stride():
    """A square zero-based HARR would hide swapped dimensions: (-1,9) must address byte 164 here."""
    path = Path("fixtures/regressions/harr-bounds-p-g2.obj")
    found = corpus.loaded(path)
    raised = mir.bodies(found, corpus.partitioned(path))
    body = raised[0][1]
    hints = raised.hints[body.entry]
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    site = min(at for at, name in found.calls.items() if name == "B$HARY")
    result = next(
        op.results[0]
        for block in body.blocks
        for op in block.ops
        if op.at == site and op.kind is mir.Kind.ADD and hints.origin_of(op.defines[0]) is not None
    )
    # Real PDS descriptor at 6; R and C at 28/30. Change the memory supplied
    # to the raised expression, not the object's compiler-generated bytes.
    memory = {6: 100, 22: 4, 24: 6, 26: -3, 28: -1, 30: 9}

    def evaluate(arg):
        match arg:
            case mir.Const(n=number):
                return number
            case mir.Cell(ref=ref):
                return memory[ref.addr.disp]
            case mir.Held(value=value):
                op = definitions[value]
                operands = list(map(evaluate, op.args))
                match op.kind:
                    case mir.Kind.LOAD | mir.Kind.COPY:
                        return operands[0]
                    case mir.Kind.SUB:
                        return operands[0] - operands[1]
                    case mir.Kind.MUL:
                        return operands[0] * operands[1]
                    case mir.Kind.ADD:
                        return operands[0] + operands[1]
        raise AssertionError(arg)

    assert evaluate(result) == 164
