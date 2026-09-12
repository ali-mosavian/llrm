from pathlib import Path

from iced_x86 import Register

from tests import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.frontend import blocks


def test_c_false_return_reaches_ax() -> None:
    # r_walk returned stale x87 status instead of false: 351 polygons rather than 258.
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    name, body = next(
        (name, body) for name, body in mir.bodies(module, list(blocks.partition(module, mapped))) if body.entry == 0
    )
    low = lower.lowered(name, body, module.calls, None, {})
    producer = next(one for one in low.insns if one.at == 0x2CE)
    consumer = next(one for one in low.insns if one.at == 0x2D6)
    returned = [(held, register) for held, register in consumer.requires if register == Register.AX]
    assert len(returned) == 1
    assert returned[0][0].value in producer.defines
    assert returned[0][0].width == 2
    assert not consumer.what.sources


def test_native_far_return_keeps_cleanup_and_both_result_words() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    name, body = next(
        (name, body) for name, body in mir.bodies(module, list(blocks.partition(module, mapped))) if body.entry == 0x6CB
    )
    low = lower.lowered(name, body, module.calls, None, {})
    consumer = next(one for one in low.insns if one.at == 0x6E0)
    assert consumer.what.name == "retf"
    assert consumer.what.sources == (ir.Imm(4, 2),)
    assert {Register.AX, Register.DX} <= {register for _, register in consumer.requires}


def test_native_leaf_preserves_unused_callee_saved_values() -> None:
    # R_WALK_LAYOUT_OK has no SI/DI saves; allocation must not acquire them as scratch.
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    name, body = next(
        (name, body) for name, body in mir.bodies(module, list(blocks.partition(module, mapped))) if body.entry == 0x6CB
    )
    low = lower.lowered(name, body, module.calls, None, {})
    consumer = next(one for one in low.insns if one.at == 0x6E0)
    defined = {value for one in low.insns for value in one.defines}
    preserved = {register: held for held, register in consumer.requires if register in (Register.SI, Register.DI)}
    assert set(preserved) == {Register.SI, Register.DI}
    assert all(held.value not in defined for held in preserved.values())
