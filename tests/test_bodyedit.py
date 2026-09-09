"""
A whole Body, replaced rather than widened in place -- proved with the
smallest possible replacement, one inserted nop.
"""

from pathlib import Path

import corpus
from qbopt.model import ir
from qbopt.objectfile import omf
from qbopt.frontend.extent import Body
from qbopt.optimize.bodyedit import edit
from qbopt.frontend.declen import decode
from qbopt.objectfile.relocate import Shift
from qbopt.frontend.extent import BodyKind
from qbopt.optimize.bodyedit import rewritten
from qbopt.optimize.bodyedit import insert_nop

MAIN = Body(BodyKind.MAIN, 0x30, None, ((0x30, 0x50), (0x60, 0x80)))


def _opaque(code: bytes, at: int) -> ir.Opaque:
    insn = decode(code, at)
    assert insn is not None
    return ir.Opaque(insn, ir.NO_EFFECT)


def test_an_insertion_inside_one_range_is_accepted() -> None:
    made = edit(MAIN, 0x40, 0x40, b"\x90")
    assert not isinstance(made, str), made
    assert made.lo == made.hi == 0x40
    assert made.data == b"\x90"


def test_a_replacement_ending_exactly_on_a_ranges_trailing_edge_is_accepted() -> None:
    # Only a *pure insertion* on an edge is ambiguous. A real replacement that
    # ends there covers real bytes the range actually owns -- nothing to
    # confuse it with whatever borders the range.
    made = edit(MAIN, 0x48, 0x50, b"\x90" * 8)
    assert not isinstance(made, str), made


def test_an_insertion_outside_every_range_is_refused() -> None:
    made = edit(MAIN, 0x55, 0x55, b"\x90")
    assert isinstance(made, str)
    assert "not inside" in made


def test_an_insertion_straddling_two_ranges_is_refused() -> None:
    # A body's own gap belongs to whatever sits between its ranges -- another
    # procedure, or a table -- never to the body itself.
    made = edit(MAIN, 0x48, 0x68, b"\x90" * 0x20)
    assert isinstance(made, str)
    assert "not inside" in made


def test_an_insertion_exactly_on_a_ranges_trailing_edge_is_refused() -> None:
    # Ambiguous with whatever comes right after -- another body's own range,
    # a gap, a table -- so refused rather than guessed at.
    made = edit(MAIN, 0x50, 0x50, b"\x90")
    assert isinstance(made, str)
    assert "range edge" in made


def test_an_insertion_exactly_on_a_ranges_leading_edge_is_refused() -> None:
    # Placed in front of whatever a branch targeting that offset lands on --
    # measured on fixtures/omf/*-evt.obj, whose main body's entry jump targets
    # its second range's own first byte exactly.
    made = edit(MAIN, 0x60, 0x60, b"\x90")
    assert isinstance(made, str)
    assert "range edge" in made


def test_a_backwards_span_is_refused() -> None:
    made = edit(MAIN, 0x40, 0x30, b"")
    assert isinstance(made, str)
    assert "backwards" in made


def test_insert_nop_skips_the_bodys_own_first_node(fixtures: Path) -> None:
    # The entry point never moves: MAIN's is blocks.ENTRY, fixed by the
    # runtime header, and a PROCEDURE's is its own PUBDEF.
    found = corpus.loaded(fixtures / "jumps-q-O.obj")
    assert found is not None
    decoded = corpus.bodies(fixtures / "jumps-q-O.obj")
    assert not isinstance(decoded, str), decoded
    main = next(b for b in decoded if b.body.kind is BodyKind.MAIN)
    made = insert_nop(main)
    assert not isinstance(made, str), made
    assert made.lo != main.body.ranges[0][0]


def test_insert_nop_lands_inside_the_bodys_own_second_range_under_evt(fixtures: Path) -> None:
    # main.ranges is ((0x30, 0x32), (0x42, ...)) under /V /W: a jmp short over
    # the event-poll stub, landing exactly on the second range's own first
    # byte. edit()'s leading-edge refusal must not strand insert_nop there --
    # it has to keep looking past the second range's own first node too.
    found = corpus.loaded(fixtures / "jumps-p-evt.obj")
    assert found is not None
    decoded = corpus.bodies(fixtures / "jumps-p-evt.obj")
    assert not isinstance(decoded, str), decoded
    main = next(b for b in decoded if b.body.kind is BodyKind.MAIN)
    assert main.body.ranges[1][0] == 0x42  # the known layout this test relies on
    made = insert_nop(main)
    assert not isinstance(made, str), made
    assert made.lo not in (0x30, 0x42)


def test_insert_nop_skips_an_inline_table_at_the_second_node() -> None:
    # Never measured in the real corpus -- the earliest Data node in any
    # fixture body is at node index 3 -- but nothing rules it out for a body
    # this commit has not seen, and a table's own start is never a real
    # instruction: taking it as a candidate would build an Edit relocate()
    # can only refuse downstream.
    code = bytes([0x90]) * 0x60
    node0, node2 = _opaque(code, 0x40), _opaque(code, 0x45)
    table = ir.Data(0x41, 0x45, ir.TableKind.MAP, (), ir.NO_EFFECT)
    body = Body(BodyKind.MAIN, 0x40, None, ((0x40, 0x50),))
    made = insert_nop(ir.BodyIR(body, (node0, table, node2)))
    assert not isinstance(made, str), made
    assert made.lo == 0x45


def test_the_object_grows_by_exactly_one_byte(fixtures: Path) -> None:
    data = (fixtures / "jumps-q-O.obj").read_bytes()
    out = rewritten(data, BodyKind.MAIN)
    before = omf.code_segment(omf.parse(data))
    after = omf.code_segment(omf.parse(out))
    assert before is not None and after is not None
    assert after[2] == before[2] + 1


def test_every_fixture_body_takes_the_insertion_and_relocates_cleanly(fixtures: Path) -> None:
    # Not every fixture body is expected to take an insertion -- a body with
    # only one node, or every candidate landing on an edge, has nothing safe
    # to edit -- but the whole point of this sweep is that refusal is rare:
    # measured over the real corpus, every one of the 171 decodable bodies
    # across 125 objects takes it -- 154 across 110 before the fpemu fixtures. Every taken edit is checked for real: the
    # rewritten records re-parse, every self-relative branch lands where
    # shift.at() says it should, and the segment grows by exactly one byte.
    taken = refused = 0
    for path in sorted(fixtures.glob("*.obj")):
        found = corpus.loaded(path)
        if found is None:
            continue
        decoded = corpus.bodies(path)
        if isinstance(decoded, str):
            continue
        for body_ir in decoded:
            made = insert_nop(body_ir)
            if isinstance(made, str):
                refused += 1
                continue
            moved = corpus.relocated(path, Shift.of([made]))
            assert not isinstance(moved, str), (path.name, body_ir.body.kind, moved)
            emitted = b"".join(record.emit() for record in moved)
            after = omf.code_segment(omf.parse(emitted))
            before = omf.code_segment(found.records)
            assert before is not None and after is not None
            assert after[2] == before[2] + 1, (path.name, body_ir.body.kind)
            taken += 1
    assert taken == 583, taken
    assert refused == 0, refused
