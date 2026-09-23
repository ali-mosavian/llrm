"""Runtime integer inputs need no concrete constant to prove exact conversion."""

from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt.model import mir
from qbopt.optimize import transform
from qbopt.analysis import floatbounds
from qbopt.model.passes import Options
from qbopt.model.floating import Format
from qbopt.model.floating import Rounding
from qbopt.model.floating import Precision
from qbopt.model.floating import Semantics


def _assert_shared_conversions(found, instructions):
    """One conversion per runtime input, whether the input loop is unrolled or not."""
    reads = sorted(at for at, name in found.calls.items() if name in ("B$RDI2", "B$RDI4"))
    assert reads, "the fixture must still consume runtime inputs"
    for index, start in enumerate(reads):
        end = reads[index + 1] if index + 1 < len(reads) else float("inf")
        region = [str(one.insn) for one in instructions if start < one.at < end]
        assert sum(one.startswith("fild ") for one in region) == 1
        assert sum(one.startswith("fst ") for one in region) == 1
        assert sum(one.startswith("fstp ") for one in region) == 1


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_fpdeep_reuses_proven_finite_array_loads(tag):
    """FPDEEP loaded p(i) five times despite its three initialized finite elements."""
    path = Path(f"fixtures/omf/fpdeep-{tag}.obj".lower())
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = transform.applied(
        mir.bodies(found, partition)[0][1], found.dgroup, found.calls, blocks=partition, found=found
    )
    loads = [
        op
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.FLOAD and op.loads and op.loads[0].base is not None
    ]
    assert not loads
    printed = [
        op.args[0].n
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.ARG and len(op.args) == 1 and isinstance(op.args[0], mir.Const) and op.args[0].width == 4
    ]
    expected = [144, 6, 512, 784, 14, 768, 3600, 30, 896]
    assert printed == expected + ([144, 6] if tag == "q-O" else [])
    from qbopt import wholeseg

    emitted = wholeseg.emitted(path.read_bytes())
    assert emitted.outcome is wholeseg.Emission.LIR, emitted.reason
    instructions = [str(one.insn) for block in corpus.partitioned(emitted.data) for one in block.insns]
    assert "fld dword ptr [si]" not in instructions
    assert not any(one.startswith(("fmul dword ptr [si]", "fadd dword ptr [si]")) for one in instructions)


# Not 0.5: every answer from it is exact, and folding an exact value needs no integer bound.
@pytest.mark.parametrize("bits", [0x7F800000, 0x7FC00000, 1])
def test_array_reuse_requires_every_element_to_have_proven_integer_bounds(bits):
    """An infinite, NaN or denormal p(1) is not a finite-integer proof."""
    path = Path("fixtures/omf/fpdeep-p-g2.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = mir.bodies(found, partition)[0][1]
    first = body.blocks[0].ops[0]
    assert first.args == (mir.Const(0x41400000, 4),)
    body = replace(
        body,
        blocks=tuple(
            replace(
                block, ops=tuple(replace(op, args=(mir.Const(bits, 4),)) if op is first else op for op in block.ops)
            )
            for block in body.blocks
        ),
    )
    changed = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    # A second read of p(i) with nothing written since is the first one's value.
    first = set()
    for block in body.blocks:
        read = set()
        for op in block.ops:
            if op.stores or op.barrier or op.kind is mir.Kind.CALL:
                read = set()
            if op.kind is mir.Kind.FLOAD and op.loads and op.loads[0].base is not None:
                if op.loads[0] not in read:
                    first.add(op.id)
                read.add(op.loads[0])
    assert first
    kept = {op.id for block in changed.blocks for op in block.ops if op.kind is mir.Kind.FLOAD}
    assert first <= kept


@pytest.mark.parametrize("guard", [None, "missing", "alignment", "segment", "wrap"])
def test_finite_array_proof_requires_known_aligned_nonwrapping_bytes(guard):
    from qbopt.analysis import ranges
    from qbopt.analysis import floatfacts

    path = Path("fixtures/omf/fpdeep-p-g2.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    body = transform.applied(
        mir.bodies(found, partition)[0][1],
        found.dgroup,
        found.calls,
        blocks=partition,
        found=found,
        options=Options(unroll=False),
    )  # Unrolled, i is a constant.
    block, index, op = next(
        (block, index, op)
        for block in body.blocks
        for index, op in enumerate(block.ops)
        if op.kind is mir.Kind.FLOAD and op.loads and op.loads[0].base is not None
    )
    memory = floatfacts.cells(body, found.dgroup, found.calls)[block.at, index]
    scoped = ranges.bounded(body)[block.at]
    definitions = {value: one for block in body.blocks for one in block.ops for value in one.defines}
    arg = op.args[0]
    if guard == "missing":
        memory = {}
    elif guard == "alignment":
        definitions = {}
    elif guard == "segment":
        arg = mir.Cell(replace(arg.ref, segment=mir.Value(99999, 0)))
    elif guard == "wrap":
        scoped = {arg.ref.base: ranges.Interval(0, 65535, 2)}
    assert floatbounds._memory(arg, op.floating.inputs[0], memory, scoped, definitions) == (
        (12, 60) if guard is None else None
    )


def test_integer_helper_with_a_live_clobbered_result_keeps_it_defined(monkeypatch):
    """B$FIL2 sign-extends into DX; replacing it must not discard a live DX result."""
    from iced_x86 import Register

    from qbopt.abi import runtime
    from qbopt.frontend import raising_float_calls

    path = Path("fixtures/regressions/fpi2cs-p-g2.obj")
    found = corpus.loaded(path)
    contracts = runtime.for_module(found)
    raise_calls = raising_float_calls.raised
    monkeypatch.setattr(raising_float_calls, "raised", lambda body, *args: body)
    bodies = mir.bodies(found, corpus.partitioned(path), contracts)
    public = bodies[0][1]
    body = mir._with_hints(public, bodies.hints[public.entry])
    block = next(one for one in body.blocks if any(found.calls.get(op.at) == "B$FIL2" for op in one.ops))
    index = next(index for index, op in enumerate(block.ops) if found.calls.get(op.at) == "B$FIL2")
    call = block.ops[index]
    result = next(value for value in call.defines if body.origin.get(value) == Register.EDX)
    observer = block.ops[index + 1]
    changed = replace(
        block, ops=tuple(replace(op, uses=(*op.uses, result)) if op is observer else op for op in block.ops)
    )
    body = replace(body, blocks=tuple(changed if one is block else one for one in body.blocks))
    raised = raise_calls(body, found, contracts)
    ops = [op for one in raised.blocks for op in one.ops]
    defined = next(op for op in ops if result in op.defines)
    assert defined.kind is mir.Kind.CALL or defined.kind is mir.Kind.EXTRACT, f"dx is defined by {defined.kind}"
    signed = next((op for op in ops if op.kind is mir.Kind.SIGN_EXTEND and set(op.defines) & set(defined.uses)), None)
    assert defined.kind is mir.Kind.CALL or signed is not None, "dx is not the sign of the word B$FIL2 was handed"


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_computed_runtime_integer_uses_one_conversion(tag):
    """FPCALC recomputed input+1 and called B$FIL2 twice instead of sharing its converted value."""
    from qbopt import wholeseg
    from qbopt.objectfile import module, omf
    path = Path(f"fixtures/regressions/fpcalc-{tag}.obj".lower())
    result = wholeseg.emitted(path.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    assert "B$FIL2" not in found.calls.values()
    instructions = [one for block in corpus.partitioned(result.data) for one in block.insns]
    _assert_shared_conversions(found, instructions)


@pytest.mark.parametrize("change", ["unknown", "writes", "control", "inputs"])
def test_helper_conversion_respects_its_effect_contract(change):
    from qbopt.abi import runtime

    path = Path("fixtures/regressions/fpicse-p-g2.obj")
    found = corpus.loaded(path)
    contracts = runtime.for_module(found)
    alterations = {
        "unknown": {"established": False},
        "writes": {"writes": runtime.Memory.ANY},
        "control": {"enters_user_code": True},
        "inputs": {"inputs": None},
    }
    contracts = {
        at: replace(rule, **alterations[change]) if found.calls.get(at) == "B$FILD" else rule
        for at, rule in contracts.items()
    }
    body = mir.bodies(found, corpus.partitioned(path), contracts)[0][1]
    assert (
        sum(
            op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$FILD" for block in body.blocks for op in block.ops
        )
        == 2
    )


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program,helper", [("fpicse", "B$FILD"), ("fpi2cs", "B$FIL2")])
def test_runtime_integer_conversion_is_shared_in_emitted_code(tag, program, helper):
    """FPICSE/FPI2CS paid two conversion calls for the same READ value across assignments."""
    from qbopt import wholeseg
    from qbopt.objectfile import module, omf
    path = Path(f"fixtures/regressions/{program}-{tag}.obj".lower())
    result = wholeseg.emitted(path.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    assert helper not in found.calls.values()
    instructions = [one for block in corpus.partitioned(result.data) for one in block.insns]
    _assert_shared_conversions(found, instructions)
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


@pytest.mark.parametrize(
    "kind,bounds,expected",
    [
        (mir.Kind.FADD, ((-32768, 32767), (-32768, 32767)), (-65536, 65534)),
        (mir.Kind.FMUL, ((-32768, 32767), (-32768, 32767)), None),
        (mir.Kind.FMUL, ((-100, 100), (-100, 100)), (-10000, 10000)),
        (mir.Kind.FSUB, ((-10, 10), (-10, 10)), (-20, 20)),
        (mir.Kind.FDIV, ((1, 3), (1, 3)), None),
        (mir.Kind.FADD, ((2**24, 2**24), (1, 1)), None),
    ],
)
def test_dynamic_arithmetic_requires_exactness_at_every_precision(kind, bounds, expected):
    rule = Semantics((Format.EXTENDED80,) * 2, Format.EXTENDED80, Precision.DYNAMIC, Rounding.DYNAMIC)
    assert floatbounds.evaluated(kind, rule, bounds) == expected
