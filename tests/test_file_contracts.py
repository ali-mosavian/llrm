"""Qrender script loading refused FREEFILE (0a23) and fixed-string load (0a3f)."""

from dataclasses import replace
from pathlib import Path

import corpus
from qbopt.backend import lower
from qbopt.model import mir

import pytest

from qbopt.abi import runtime


@pytest.mark.parametrize("procedure,at,cleanup", [
    ("SYS_PARSE_ARGS", 0x12f, 2), ("SYS_PARSE_ARGS", 0x823, None),
    ("SYS_INIT_TABLES", 0x8f5, 0),
    ("SYS_ERROR", 0x980, None), ("SYS_ERROR", 0x989, 4),
    ("SYS_ERROR", 0x9a2, 4), ("SYS_TIME_INIT", 0x9ca, 0),
    ("SYS_MEM_MARK", 0xdc3, 2), ("SYS_MEM_MARK", 0xe22, 4),
])
def test_qrender_sys_lowers_runtime_calls(procedure: str, at: int, cleanup: int | None) -> None:
    """SYS emitted no optimized OBJ: string, power, screen, clock and heap calls refused."""
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
                for name, cleanup in (("HOST_SHUTDOWN", 0), ("COM_TOKENIZE", 8),
                                      ("QGLMEMAVAIL", 2), ("QGLTMRINIT", 2),
                                      ("QGLTMRTICKS", 0), ("QGLTMRCYCLES", 0),
                                      ("QGLKBDSHUTDOWN", 0), ("QGLMOUSESHUTDOWN", 0),
                                      ("QGLTMRSHUTDOWN", 0), ("QGLVGASHUTDOWN", 0),
                                      ("QGLMEMSHUTDOWN", 0))}
    rules = runtime.for_module(found, external=external)
    name, body = next((name, body) for name, body in mir.bodies(found, corpus.partitioned(path), rules)
                      if name == f"procedure {procedure}")
    block = next(block for block in body.blocks if any(op.at == at for op in block.ops))
    body = replace(body, entry=block.at, blocks=(block,))
    lowered = lower.lowered(name, body, found.calls, found.absorbed, rules)
    assert any(one.at == at for one in lowered.insns)
    contract = rules[at]
    assert contract.cleanup == cleanup
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
    ("B$CSCN", None), ("B$WIDT", 4), ("B$SLEP", 4),
    ("B$TIMR", 0), ("B$FRI2", 2), ("B$STI4", 4),
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


@pytest.fixture(scope="module")
def model_calls(request):
    path = Path(f"fixtures/regressions/qrender-{request.param}-v-g3.obj")
    found = corpus.loaded(path)
    rules = runtime.for_module(found)
    return found, rules, list(mir.bodies(found, corpus.partitioned(path), rules))


@pytest.mark.parametrize("model_calls,symbol", [
    ("d-mdl", symbol) for symbol in ("B$FLOF", "B$GET3", "B$GET4", "B$SACT")
] + [
    ("d-surf", symbol) for symbol in ("B$DSG0", "B$PUT3", "B$SMID", "B$SPAC")
] + [
    ("pl-move", symbol) for symbol in ("B$RND0", "B$ATN4", "B$UBND")
] + [
    ("mod-tex", symbol) for symbol in ("B$SSEK", "B$STRI")
], indirect=["model_calls"])
def test_model_file_calls_lower_without_replacement(model_calls, symbol):
    """Qrender modules emitted no optimized OBJ: real runtime interfaces were unknown."""
    found, rules, bodies = model_calls
    seen = 0
    for name, body in bodies:
        for block in body.blocks:
            for op in block.ops:
                if found.calls.get(op.at) != symbol:
                    continue
                isolated = replace(body, entry=block.at,
                    blocks=(replace(block, phis=(), ops=(op,), succ=()),))
                result = lower.lowered(name, isolated, found.calls, found.absorbed, rules)
                assert any(one.at == op.at and one.what.name == "call" for one in result.insns)
                seen += 1
    assert seen > 0
    rule = runtime.per_call({0: symbol}, "vbdos")[0]
    assert rule.inputs == frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX,
                                    runtime.Reg.DX, runtime.Reg.SI, runtime.Reg.DI})
    assert rule.cleanup is None and rule.clobbers == runtime.EVERY
    assert rule.reads is runtime.Memory.ANY and rule.writes is runtime.Memory.ANY
    assert rule.control is runtime.Control.UNKNOWN and rule.raises_error
    assert runtime.per_call({0: symbol}, "qb45")[0].inputs is None
