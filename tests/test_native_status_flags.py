from pathlib import Path

from iced_x86 import Register

from tests import corpus
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.frontend import blocks


def test_c_status_word_reaches_sahf_high_byte() -> None:
    # r_walk's rewritten SAHF used MOV AL,DL, leaving AH's comparison bits stale.
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    name, body = next(
        (name, body) for name, body in mir.bodies(module, list(blocks.partition(module, mapped))) if body.entry == 0
    )
    low = lower.lowered(name, body, module.calls, module.absorbed, {})
    consumer = next(one for one in low.insns if one.at == 0x60)
    producer = next(one for one in low.insns if one.at == 0x5D)
    assert len(consumer.requires) == 1
    held, register = consumer.requires[0]
    assert held.value in producer.defines
    # The SSA value is the entire status word, not its shifted high byte.
    assert held.width == 2
    assert register == Register.AX
