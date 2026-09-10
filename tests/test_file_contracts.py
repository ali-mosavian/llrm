"""Qrender script loading refused FREEFILE (0a23) and fixed-string load (0a3f)."""

import pytest

from qbopt.abi import runtime


def test_qrender_argument_parser_lowers_lowercase_call() -> None:
    """SYS_PARSE_ARGS refused at 012f (LCAS), so its optimized object could not link."""
    from dataclasses import replace
    from pathlib import Path
    import corpus
    from qbopt.model import mir
    from qbopt.backend import lower

    path = Path("fixtures/regressions/qrender-sys-v-g3.obj")
    found = corpus.loaded(path)
    external = {name: replace(runtime.worst(name), cleanup=cleanup,
                inputs=frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
                                  runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI}))
                for name, cleanup in (("HOST_SHUTDOWN", 0), ("COM_TOKENIZE", 8))}
    rules = runtime.for_module(found, external=external)
    name, body = next((name, body) for name, body in mir.bodies(found, corpus.partitioned(path), rules)
                      if name == "procedure SYS_PARSE_ARGS")
    block = next(block for block in body.blocks if any(op.at == 0x12f for op in block.ops))
    body = replace(body, entry=block.at, blocks=(block,))
    lowered = lower.lowered(name, body, found.calls, found.absorbed, rules)
    assert any(one.at == 0x12f for one in lowered.insns)
    contract = rules[0x12f]
    assert contract.cleanup == 2
    assert contract.reads is runtime.Memory.ANY and contract.writes is runtime.Memory.ANY
    assert contract.clobbers == runtime.EVERY and contract.raises_error


def test_peos_register_interface_does_not_claim_fixed_stack_cleanup():
    """Qrender INPUT epilogue at 0aa2 refused; terminal INPUT can relocate SP."""
    contract = runtime.per_call({0: "B$PEOS"}, "vbdos")[0]
    assert contract.inputs == frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
                                         runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    assert contract.cleanup is None
    assert contract.clobbers == runtime.EVERY
    assert contract.control is runtime.Control.UNKNOWN
    assert contract.writes is runtime.Memory.ANY


@pytest.mark.parametrize("name,cleanup", [
    ("B$FREF", 0), ("B$LDFS", 6), ("B$OPEN", 8), ("B$DSKI", 2),
    ("B$FEOF", 2), ("B$CLOS", None), ("B$ERAS", 2), ("B$FLEN", 2),
    ("B$FMID", 6), ("B$ASSN", 12), ("B$SCMP", 4), ("B$SCPF", 2),
    ("B$LNIN", 10), ("B$ERS1", 2),
    ("B$RTRM", 2), ("B$FASC", 2), ("B$FCHR", 2),
    ("B$LEFT", 4), ("B$RGHT", 4),
    ("B$FMKI", 2), ("B$FMKL", 4), ("B$FCVI", 2), ("B$FCVS", 2),
])
def test_vbdos_file_setup_retains_unknown_effects(name, cleanup):
    """Qrender refused string/file calls, including LEFT/RIGHT; CLOSE varies in arity."""
    contract = runtime.per_call({0: name}, "vbdos")[0]
    assert contract.cleanup == cleanup
    assert contract.inputs == frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
                                         runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    assert contract.clobbers == runtime.EVERY
    assert contract.reads is runtime.Memory.ANY
    assert contract.writes is runtime.Memory.ANY
    assert contract.control is runtime.Control.UNKNOWN
    assert contract.raises_error
