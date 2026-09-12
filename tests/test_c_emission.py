from pathlib import Path
from dataclasses import replace

from qbopt import wholeseg
from qbopt.abi import runtime


def test_native_c_reaches_production_backend() -> None:
    original = Path("fixtures/regressions/r_walk-borland.obj").read_bytes()
    # r_bsp's measured Pascal RETF 28; no memory or preservation promises.
    external = replace(
        runtime.contract("R_EMIT_ENTITIES"),
        inputs=frozenset(
            {runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}
        ),
        cleanup=28,
    )
    result = wholeseg.emitted(original, optimise=False, native_fpu=True, external_contracts={external.name: external})
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert result.data != original
