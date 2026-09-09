"""
Which body a byte belongs to: the module's own main body, a SUB/FUNCTION, or
neither.
"""

from pathlib import Path
import pytest

import corpus
from qbopt.objectfile import module
from qbopt.frontend.extent import Body
from qbopt.frontend.extent import BodyKind
from qbopt.frontend.extent import Partition
from qbopt.frontend.extent import partition


@pytest.mark.parametrize("tag,entry", [("p-evt", 0xFA), ("v-evt", 0xF0)])
def test_timer_handler_has_its_own_entry(tag, entry):
    """EVTRAP's handler was assigned main's SSA by falling through END."""
    found = module.load(Path(f"fixtures/regressions/evtrap-{tag}.obj"))
    result = partition(found)
    assert not isinstance(result, str), result
    handler = next(body for body in result.bodies if body.seed == entry)
    assert handler.kind.value == "event-handler"
    assert not any(lo <= entry < hi for body in result.bodies
                   if body.kind is BodyKind.MAIN for lo, hi in body.ranges)


def test_empty_statement_table_is_data_not_a_handler_instruction():
    """VBDOS EVTRAP's OF_STA sentinel at 0116 was decoded as ADD [BX+SI],AL."""
    from qbopt.frontend.blocks import code_map
    found = module.load(Path("fixtures/regressions/evtrap-v-evt.obj"))
    mapped = code_map(found)
    assert not isinstance(mapped, str), mapped
    assert (0x116, 0x118) in mapped.tables
    assert 0x116 not in mapped.starts
    assert partition(found).complete


def test_emission_preserves_the_empty_statement_table():
    """ADDRM VBDOS refused OF_STA at 00bc when layout omitted trailing data."""
    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.frontend.blocks import empty_statement_table
    result = wholeseg.emitted(Path("fixtures/omf/addrm-v-g3.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    assert empty_statement_table(found) is not None


def test_every_fixture_partitions_completely(obj: Path) -> None:
    # Measured, not assumed: every one of the 110 real objects here accounts
    # for its whole code segment as the module's own main body plus its
    # SUB/FUNCTIONs plus, where /V or /W built one, the event-poll stub --
    # nothing left unexplained and no byte owned twice.
    found = corpus.loaded(obj)
    assert found is not None
    found_partition = corpus.extents(obj)
    assert not isinstance(found_partition, str), found_partition
    assert found_partition.complete, (found_partition.unexplained, found_partition.conflicts)


def _body(found_partition: Partition | str, kind: BodyKind) -> list[Body]:
    assert isinstance(found_partition, Partition)
    return [b for b in found_partition.bodies if b.kind is kind]


def test_procs_v_g3_bodies_match_the_measured_layout(fixtures: Path) -> None:
    # BC lays TWICE& and REPORT out exactly where suite/procs.bas declares
    # them, and jumps around each to keep the main body's own flow going --
    # so the main body is three disjoint ranges, not one contiguous prefix:
    # the statements before TWICE&, a 3-byte trampoline BC placed *between*
    # the two procedures to skip REPORT too, and the implicit END statement's
    # own call, sitting *after* both. Confirmed against CodeView in
    # test_extent_cv.py.
    found = corpus.loaded(fixtures / "procs-v-g3.obj")
    assert found is not None
    found_partition = corpus.extents(fixtures / "procs-v-g3.obj")
    assert not isinstance(found_partition, str)
    assert found_partition.complete

    (main,) = _body(found_partition, BodyKind.MAIN)
    assert main.ranges == ((0x30, 0xEA), (0x11B, 0x11E), (0x14C, 0x153))

    procs = {b.name: b.ranges for b in _body(found_partition, BodyKind.PROCEDURE)}
    assert procs == {"TWICE": ((0xEA, 0x11B),), "REPORT": ((0x11E, 0x14C),)}


def test_procedure_ptot_has_no_runtime_call_but_still_partitions(fixtures: Path) -> None:
    # PDS 7.1's /Ot emits a plain push bp/pop bp prologue and epilogue, no
    # B$ENRA/B$EXSA call in either direction -- PUBDEF is the only signal
    # both shapes agree on, and this is the fixture that tests it.
    found = corpus.loaded(fixtures / "procs-p-ot.obj")
    assert found is not None
    assert not set(found.calls.values()) & {"B$ENRA", "B$EXSA"}
    found_partition = corpus.extents(fixtures / "procs-p-ot.obj")
    assert not isinstance(found_partition, str)
    assert found_partition.complete
    names = {b.name for b in _body(found_partition, BodyKind.PROCEDURE)}
    assert names == {"TWICE", "REPORT"}


def test_event_stub_is_its_own_body_not_a_gap(fixtures: Path) -> None:
    # Nothing in the main body's own control flow falls into the /V-/W stub
    # (that is the whole reason blocks.event_stub() exists), so it needs its
    # own seed or its bytes come back "unexplained".
    found = corpus.loaded(fixtures / "arith-v-evt.obj")
    assert found is not None
    found_partition = corpus.extents(fixtures / "arith-v-evt.obj")
    assert not isinstance(found_partition, str)
    assert found_partition.complete
    (stub,) = _body(found_partition, BodyKind.EVENT_STUB)
    assert stub.ranges == ((0x32, 0x42),)


def test_qb45_under_evt_has_no_stub_body(fixtures: Path) -> None:
    # QuickBASIC 4.5 sets the same U_FLAG bits under /V /W but emits no stub.
    found = corpus.loaded(fixtures / "arith-q-evt.obj")
    assert found is not None
    found_partition = corpus.extents(fixtures / "arith-q-evt.obj")
    assert not isinstance(found_partition, str)
    assert found_partition.complete
    assert not _body(found_partition, BodyKind.EVENT_STUB)


def test_the_resume_map_fallthrough_does_not_leak_module_targets(fixtures: Path) -> None:
    # divmod-v-g3.obj's /X RESUME map sits right where an unrelated call
    # (B$CENP, the program's own implicit END) happens to end, and gets
    # classified as a TABLE block the same way an ON GOTO table would -- but
    # Block.succ for it is every fixup target in the module, not real control
    # flow. Trusting that blindly would make this single-body module's main
    # body "reach" its own procedure seeds in a module that had any; here,
    # with none, the regression this guards is simpler: the two trailing
    # bytes right after the map are still claimed, not left unexplained.
    found = corpus.loaded(fixtures / "divmod-v-g3.obj")
    assert found is not None
    assert not found.publics  # no SUB/FUNCTION in this module
    found_partition = corpus.extents(fixtures / "divmod-v-g3.obj")
    assert not isinstance(found_partition, str)
    assert found_partition.complete
    (main,) = found_partition.bodies
    assert main.ranges[-1][1] == found.end


def test_a_module_with_no_header_is_refused_not_guessed(fixtures: Path) -> None:
    found = corpus.loaded(fixtures / "procs-v-g3.obj")
    assert found is not None
    headerless = module.Module(found.records, found.seg, found.name, found.code[0x30:], 0, len(found.code) - 0x30)
    found_partition = partition(headerless)
    assert isinstance(found_partition, str)
