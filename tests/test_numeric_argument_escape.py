"""Passing an integer's contents to PRINT does not expose its address."""

from pathlib import Path

import corpus
import pytest

from qbopt.objectfile import module


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_read_destination_escapes_across_segment_setup(tag):
    """MEMPHI printed 10 twice instead of 10,9: READ's address crossed PUSH DS/POP ES before PUSH BX."""
    found = corpus.loaded(Path(f"fixtures/regressions/memphi-{tag}.obj".lower()))
    assert {(found.program_data, 6), (found.program_data, 8)} <= module.escaped(found)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_long_arithmetic_values_do_not_escape_chain_inputs(tag):
    """CHAIN retained seven IDIVs because nested numeric arguments made a and b appear escaped."""
    from qbopt import wholeseg
    from qbopt.model import mir
    path = Path(f"fixtures/regressions/chain5-{tag}.obj".lower())
    found = corpus.loaded(path)
    assert not any(segment == found.program_data for segment, _ in module.escaped(found))
    states = []
    def watch(stage, name, body):
        if stage == "mir-widen":
            states.append(body)
    result = wholeseg.emitted(path.read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    # Frame temporaries still lose facts across PRINT independently of this
    # global-address escape bug; their four divisions remain for now.
    assert sum(op.kind is mir.Kind.DIVMOD for body in states for block in body.blocks for op in block.ops) <= 4


@pytest.mark.parametrize("kind", ["unknown", "local", "unestablished"])
def test_arithmetic_argument_proof_requires_the_established_external_helper(kind, monkeypatch):
    """A same-named local helper or unknown callee cannot inherit the runtime's numeric ABI."""
    from dataclasses import replace
    from qbopt.abi import runtime
    found = corpus.loaded(Path("fixtures/omf/chain-p-g2.obj"))
    names = {"B$MUI4", "B$DVI4", "B$RMI4", "B$CPI4"}
    if kind == "unknown":
        found = replace(found, calls={at: "UNKNOWN" if name in names else name for at, name in found.calls.items()})
    elif kind == "local":
        monkeypatch.setattr(module, "defines", lambda *args: names)
    else:
        original = runtime.contract
        monkeypatch.setattr(runtime, "contract", lambda name: replace(original(name), established=False)
                            if name in names else original(name))
    assert any(segment == found.program_data for segment, _ in module.escaped(found))


def test_qb_register_materialized_descriptor_addresses_escape():
    """QB FPDEEP reported no escapes although MOV AX,OFFSET descriptor; PUSH AX passes seven strings."""
    found = corpus.loaded(Path("fixtures/omf/fpdeep-q-O.obj".lower()))
    assert {(9, offset) for offset in (16, 22, 28, 38, 58, 66, 78)} <= module.escaped(found)


@pytest.mark.parametrize("tag", ["p-g2", "v-g3"])
def test_local_copy_address_is_not_a_call_argument(tag):
    """FPDEEP materializes its DOUBLE initializer address for MOVSW, not for a runtime argument."""
    found = corpus.loaded(Path(f"fixtures/omf/fpdeep-{tag}.obj".lower()))
    assert not any(segment == found.program_data for segment, _ in module.escaped(found))


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program", ["arith", "nots", "fpcse"])
def test_numeric_prints_do_not_escape_program_variables(tag, program):
    """ARITH/NOTS/FPCSE lost constants because PUSH [r] was interpreted as PUSH OFFSET r."""
    found = corpus.loaded(Path(f"fixtures/omf/{program}-{tag}.obj".lower()))
    assert not any(segment == found.program_data for segment, _ in module.escaped(found))


@pytest.mark.parametrize("name", ["UNKNOWN", "B$PSSD", "B$PEI2"])
def test_other_consumers_do_not_grant_a_long_value_proof(name):
    """Unknown, pointer-taking and wrong-width callees cannot justify a long argument group."""
    from dataclasses import replace
    found = corpus.loaded(Path("fixtures/omf/nots-p-g2.obj"))
    calls = {at: name if routine == "B$PEI4" else routine for at, routine in found.calls.items()}
    found = replace(found, calls=calls)
    assert any(segment == found.program_data for segment, _ in module.escaped(found))


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_the_escape_scan_decodes_from_instructions_not_the_header(tag):
    """FPCALC printed its first READ three times on /G3. A sweep from offset 0 decoded the
    module header as code and never landed on `mov bx,offset inputValue`, so READ was
    taken to write nothing the program owns and the unrolled reloads were forwarded."""
    found = corpus.loaded(Path(f"fixtures/regressions/fpcalc-{tag}.obj".lower()))
    assert (found.program_data, 6) in module.escaped(found)
