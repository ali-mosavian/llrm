"""Passing an integer's contents to PRINT does not expose its address."""

from pathlib import Path

import corpus
import pytest

from qbopt.objectfile import module


def test_qb_register_materialized_descriptor_addresses_escape():
    """QB FPDEEP reported no escapes although MOV AX,OFFSET descriptor; PUSH AX passes seven strings."""
    found = corpus.loaded(Path("fixtures/omf/fpdeep-q-O.obj"))
    assert {(9, offset) for offset in (16, 22, 28, 38, 58, 66, 78)} <= module.escaped(found)


@pytest.mark.parametrize("tag", ["p-g2", "v-g3"])
def test_local_copy_address_is_not_a_call_argument(tag):
    """FPDEEP materializes its DOUBLE initializer address for MOVSW, not for a runtime argument."""
    found = corpus.loaded(Path(f"fixtures/omf/fpdeep-{tag}.obj"))
    assert not any(segment == found.program_data for segment, _ in module.escaped(found))


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program", ["arith", "nots", "fpcse"])
def test_numeric_prints_do_not_escape_program_variables(tag, program):
    """ARITH/NOTS/FPCSE lost constants because PUSH [r] was interpreted as PUSH OFFSET r."""
    found = corpus.loaded(Path(f"fixtures/omf/{program}-{tag}.obj"))
    assert not any(segment == found.program_data for segment, _ in module.escaped(found))


@pytest.mark.parametrize("name", ["UNKNOWN", "B$PSSD", "B$PEI2"])
def test_other_consumers_do_not_grant_a_long_value_proof(name):
    """Unknown, pointer-taking and wrong-width callees cannot justify a long argument group."""
    from dataclasses import replace
    found = corpus.loaded(Path("fixtures/omf/nots-p-g2.obj"))
    calls = {at: name if routine == "B$PEI4" else routine for at, routine in found.calls.items()}
    found = replace(found, calls=calls)
    assert any(segment == found.program_data for segment, _ in module.escaped(found))
