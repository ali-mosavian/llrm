"""Runtime integer inputs need no concrete constant to prove exact conversion."""

from dataclasses import replace
from pathlib import Path

import corpus
import pytest

from qbopt.analysis import floatbounds
from qbopt.model import mir
from qbopt.model.floating import Format, Precision, Rounding, Semantics
from qbopt.optimize import transform


def test_integer_helper_with_a_live_clobbered_result_is_not_removed(monkeypatch):
    """B$FIL2 sign-extends into DX; replacing it must not discard a live DX result."""
    from iced_x86 import Register
    from qbopt.abi import runtime
    from qbopt.frontend import raising_float_calls
    path = Path("fixtures/regressions/fpi2cs-p-g2.obj")
    found = corpus.loaded(path)
    contracts = runtime.for_module(found)
    raise_calls = raising_float_calls.raised
    monkeypatch.setattr(raising_float_calls, "raised", lambda body, *args: body)
    body = mir.bodies(found, corpus.partitioned(path), contracts)[0][1]
    block = next(one for one in body.blocks if any(found.calls.get(op.at) == "B$FIL2" for op in one.ops))
    index = next(index for index, op in enumerate(block.ops) if found.calls.get(op.at) == "B$FIL2")
    call = block.ops[index]
    result = next(value for value in call.defines if body.origin.get(value) == Register.EDX)
    observer = block.ops[index + 1]
    changed = replace(block, ops=tuple(replace(op, uses=(*op.uses, result)) if op is observer else op for op in block.ops))
    body = replace(body, blocks=tuple(changed if one is block else one for one in body.blocks))
    raised = raise_calls(body, found, contracts)
    assert next(op for one in raised.blocks for op in one.ops if op.id == call.id).kind is mir.Kind.CALL


@pytest.mark.parametrize("change", ["unknown", "writes", "control", "inputs"])
def test_helper_conversion_respects_its_effect_contract(change):
    from qbopt.abi import runtime
    path = Path("fixtures/regressions/fpicse-p-g2.obj")
    found = corpus.loaded(path)
    contracts = runtime.for_module(found)
    alterations = {"unknown": {"established": False}, "writes": {"writes": runtime.Memory.ANY},
                   "control": {"enters_user_code": True}, "inputs": {"inputs": None}}
    contracts = {at: replace(rule, **alterations[change]) if found.calls.get(at) == "B$FILD" else rule
                 for at, rule in contracts.items()}
    body = mir.bodies(found, corpus.partitioned(path), contracts)[0][1]
    assert sum(op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$FILD"
               for block in body.blocks for op in block.ops) == 2


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program,helper", [("fpicse", "B$FILD"), ("fpi2cs", "B$FIL2")])
def test_runtime_integer_conversion_is_shared_in_emitted_code(tag, program, helper):
    """FPICSE/FPI2CS paid two conversion calls for the same READ value across assignments."""
    from qbopt import wholeseg
    from qbopt.objectfile import module, omf
    path = Path(f"fixtures/regressions/{program}-{tag}.obj")
    result = wholeseg.emitted(path.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    assert helper not in found.calls.values()
    instructions = [one for block in corpus.partitioned(result.data) for one in block.insns]
    assert sum(str(one.insn).startswith("fild ") for one in instructions) == 1
    assert sum(str(one.insn).startswith("fstp ") for one in instructions) == 2
    load = next(one for one in instructions if str(one.insn).startswith("fild "))
    assert found.code[load.at] == 0xcd  # Keep the object's software-FP protocol.
    body = mir.bodies(found, corpus.partitioned(result.data))[0][1]
    converted = next(op for block in body.blocks for op in block.ops if op.name == "fild")
    from qbopt.objectfile.module import Space
    assert converted.loads[0].addr.space is Space.SEGMENT
    assert converted.loads[0].addr.disp == 6


@pytest.mark.parametrize("format,expected", [(Format.SIGNED16, 1), (Format.SIGNED32, 1), (Format.BINARY32, 2)])
def test_unknown_integer_loads_share_a_value_but_unknown_floats_do_not(format, expected):
    """FPCSEX reloads its runtime input; integer conversion can share without assuming finite REALs."""
    path = Path("fixtures/omf/fpcsex-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    block = next(block for block in body.blocks if any(op.kind is mir.Kind.FLOAD for op in block.ops))
    loads = [op for op in block.ops if op.kind is mir.Kind.FLOAD][:2]
    rule = Semantics((format,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE)
    block = replace(block, ops=tuple(replace(op, floating=rule) for op in loads), phis=(), succ=())
    body = replace(body, blocks=(block,), entry=block.at, initial=())
    result = transform.subexpressions(body, found.dgroup)
    assert sum(op.kind is mir.Kind.FLOAD for one in result.blocks for op in one.ops) == expected


@pytest.mark.parametrize("kind,bounds,expected", [
    (mir.Kind.FADD, ((-32768, 32767), (-32768, 32767)), (-65536, 65534)),
    (mir.Kind.FMUL, ((-32768, 32767), (-32768, 32767)), None),
    (mir.Kind.FMUL, ((-100, 100), (-100, 100)), (-10000, 10000)),
    (mir.Kind.FSUB, ((-10, 10), (-10, 10)), (-20, 20)),
    (mir.Kind.FDIV, ((1, 3), (1, 3)), None),
    (mir.Kind.FADD, ((2**24, 2**24), (1, 1)), None),
])
def test_dynamic_arithmetic_requires_exactness_at_every_precision(kind, bounds, expected):
    rule = Semantics((Format.EXTENDED80,) * 2, Format.EXTENDED80, Precision.DYNAMIC, Rounding.DYNAMIC)
    assert floatbounds.evaluated(kind, rule, bounds) == expected
