from pathlib import Path
from dataclasses import replace

import pytest

from tests import corpus
from qbopt.model import mir
from qbopt.frontend import blocks
from qbopt.analysis import liveness
from qbopt.optimize import transform
from qbopt.frontend import raising_words


def test_culling_pointer_arithmetic_preserves_the_returned_upper_word(monkeypatch: pytest.MonkeyPatch) -> None:
    # r_walk's ADD SI,8 lost the upper-word dependency needed by POP SI
    # at return. The loop could appear to preserve bits with no producer.
    monkeypatch.setattr(raising_words, "carried", lambda body: body)
    found = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    body = next(body for _, body in mir.bodies(found, blocks.partition(found, mapped)) if body.entry == 0)
    seed = next(op for block in body.blocks for op in block.ops if op.at == 0x1C)
    source = seed.args[0]
    assert isinstance(source, mir.Held)
    assert (source.value, transform.HIGH) in transform.halves(body)


def test_culling_restore_carries_the_original_upper_word(monkeypatch: pytest.MonkeyPatch) -> None:
    # Three pointer increments remained live because POP SI carried its
    # upper word through the loop, although no word instruction changed it.
    found = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    with monkeypatch.context() as context:
        context.setattr(raising_words, "carried", lambda body: body)
        body = next(body for _, body in mir.bodies(found, blocks.partition(found, mapped)) if body.entry == 0)
    normalized = raising_words.carried(body)
    restore = next(op for block in normalized.blocks for op in block.ops if op.at == 0x2F0)
    assert len(restore.merges) == 1
    source = next(iter(restore.merges))
    assert source in liveness.entry_values(body)
    assert raising_words.carried(normalized) == normalized

    seed = next(op for block in body.blocks for op in block.ops if op.at == 0x1C)
    result = seed.results[0]
    assert isinstance(result, mir.Held)
    wide = replace(seed, results=(mir.Held(result.value, 4),), merges={})
    changed = replace(
        body,
        blocks=tuple(
            replace(block, ops=tuple(wide if op is seed else op for op in block.ops)) for block in body.blocks
        ),
    )
    stopped = raising_words.carried(changed)
    restore = next(op for block in stopped.blocks for op in block.ops if op.at == 0x2F0)
    assert set(restore.merges) == {result.value}
