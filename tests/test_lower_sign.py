"""Sign extraction need not force two fixed registers when flags are dead."""

import pytest

from qbopt.model import ir, mir
from qbopt.backend import lower


@pytest.mark.parametrize("live_flags", [False, True])
def test_sign_extraction_respects_flags_across_blocks(live_flags: bool) -> None:
    """ADDRM spilled an accumulator around CWD; SAR avoids AX/DX only with dead flags."""
    source, result = mir.Value(1, 0), mir.Value(2, 0)
    condition = mir.Value(3, 0, flags=True)
    sign = mir.Op(
        0,
        ir.Operation.EXTEND,
        "cwd",
        (result,),
        (source,),
        kind=mir.Kind.CONVERT,
        args=(mir.Held(source, 2),),
        results=(mir.Held(result, 2),),
    )
    consumer = mir.Op(
        1,
        ir.Operation.BRANCH,
        "jz",
        (),
        (condition,) if live_flags else (),
        kind=mir.Kind.BRANCH,
        target=2,
    )
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (sign,), (1,)),
            mir.MirBlock(1, (), (consumer,), (2,)),
            mir.MirBlock(2, (), (), ()),
        ),
    )
    instructions = lower.lowered("sign", body, {}, (), {}).blocks[0].insns
    assert [one.what.name for one in instructions] == (["cwd"] if live_flags else ["mov", "sar"])
    if not live_flags:
        assert all(not one.requires and not one.delivers for one in instructions)
        assert instructions[-1].what.sources[-1] == ir.Imm(15, 1)
