"""Qrender's turbulence initializer refused at the VBDOS sine entry."""

from pathlib import Path

import pytest

from qbopt import wholeseg
from qbopt.abi import runtime


@pytest.mark.parametrize("name", ["B$SIN4", "B$SIN8", "B$COS4", "B$COS8"])
def test_vbdos_trig_retains_conservative_register_and_memory_effects(name):
    """Qrender refused sine at 0x97 and camera cosine at 0x370 as unknown inputs."""
    routine = runtime.per_call({0: name}, "vbdos")[0]
    assert routine.inputs == frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
                                       runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    assert routine.cleanup == 0
    assert routine.clobbers == runtime.EVERY
    assert routine.reads is runtime.Memory.ANY
    assert routine.writes is runtime.Memory.ANY
    assert routine.raises_error


def test_qrender_turbulence_initializer_reaches_lir_emission():
    """D_INIT_TURB refused B$SIN8 at 0x97 despite its stack-neutral public interface."""
    data = Path("fixtures/regressions/qrender-dturb-v-g3.obj").read_bytes()
    result = wholeseg.emitted(data)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
