from dataclasses import replace
from pathlib import Path

import corpus
import pytest

from qbopt.model import mir
from qbopt.frontend import raising_division
from qbopt.optimize import transform
from qbopt import wholeseg


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("matching", [False, True])
def test_stride_division_requires_the_dividends_sign_extension(tag: str, matching: bool, monkeypatch) -> None:
    """STRIDE's i / 5 carried machine halves, hiding the quotient recurrence from MIR."""
    path = Path(f"fixtures/omf/stride-{tag}.obj")
    with monkeypatch.context() as context:
        context.setattr(raising_division, "scalar", lambda body: body)
        raw = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    divide = next(op for block in raw.blocks for op in block.ops if op.kind is mir.Kind.DIV)
    high, dividend, divisor = divide.args
    if not matching:
        raw = replace(raw, blocks=tuple(
            replace(block, ops=tuple(
                replace(op, args=(divisor,)) if high.value in op.defines else op
                for op in block.ops
            )) for block in raw.blocks
        ))
    result = raising_division.scalar(raw)
    changed = next(op for block in result.blocks for op in block.ops if op.at == divide.at)
    assert changed.kind is (mir.Kind.DIVMOD if matching else mir.Kind.DIV)
    if matching:
        assert changed.args == (dividend, divisor)
        assert high.value not in changed.uses
        emitted = wholeseg.emitted(path.read_bytes())
        assert emitted.outcome is wholeseg.Emission.LIR, emitted.reason


@pytest.mark.parametrize("width", [2, 4])
@pytest.mark.parametrize("divisor", [-1, 0, 1, 5])
def test_division_fault_gate_uses_the_operands_width(width: int, divisor: int) -> None:
    """An all-ones divisor is -1, whose minimum-signed quotient can fault; it is not safe to speculate."""
    masked = divisor & ((1 << (8 * width)) - 1)
    op = mir.Op(1, None, "", (), (), kind=mir.Kind.DIVMOD, args=(mir.Const(0, width), mir.Const(masked, width)))
    assert transform._cannot_fault(op) is (divisor not in (-1, 0))
