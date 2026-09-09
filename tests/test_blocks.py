"""
Which bytes are code, found by reachability rather than assumed.
"""

from pathlib import Path

import pytest

import corpus
from helpers import hx
from qbopt.objectfile import module
from qbopt.frontend.blocks import PAD
from qbopt.frontend.blocks import Ends
from qbopt.frontend.blocks import ENTRY
from qbopt.frontend.blocks import benign
from qbopt.frontend.declen import decode
from qbopt.frontend.blocks import event_stub
from qbopt.frontend.blocks import has_header
from qbopt.frontend.blocks import terminator


@pytest.mark.parametrize(
    ("enc", "ends"),
    [
        ("74 02", Ends.CONDITIONAL),
        ("E2 02", Ends.CONDITIONAL),
        ("EB 02", Ends.JUMP),
        ("E9 02 00", Ends.JUMP),
        ("C3", Ends.RETURN),
        ("CB", Ends.RETURN),
        ("CA 04 00", Ends.RETURN),
        ("EA 00 00 00 00", Ends.LEAVES),
        ("FF 26 00 00", Ends.INDIRECT),
        ("FF 2E 00 00", Ends.INDIRECT),
        # these transfer control and come back, which is not the same thing
        ("9A 00 00 00 00", Ends.FALLS_THROUGH),
        ("E8 02 00", Ends.FALLS_THROUGH),
        ("FF 16 00 00", Ends.FALLS_THROUGH),
        ("FF 1E 00 00", Ends.FALLS_THROUGH),
        ("CD 21", Ends.FALLS_THROUGH),
    ],
)
def test_what_ends_a_block(enc: str, ends: Ends) -> None:
    insn = decode(hx(enc), 0)
    assert insn is not None
    assert terminator(insn) is ends


@pytest.mark.parametrize("tag", ["p-evt", "v-evt"])
def test_runtime_return_does_not_fall_into_the_next_statement(tag: str) -> None:
    """EVTRAP's B$RETA discards its call return address, not a normal CALL."""
    from dataclasses import replace
    from qbopt.frontend.blocks import walk, partition
    found = module.load(Path(f"fixtures/omf/addrm-{tag}.obj"))
    found = replace(found, code=hx("9a 00 00 00 00 90 c3"), start=0, end=7,
                    calls={0: "B$RETA"}, targets=frozenset(), publics=frozenset())
    mapped = walk(found, 0)
    assert not isinstance(mapped, str), mapped
    assert mapped.starts == frozenset({0})
    body = partition(found, mapped)
    assert len(body) == 1
    assert body[0].ends is Ends.LEAVES
    assert body[0].succ == ()


def test_every_module_is_mapped(obj: Path) -> None:
    found = corpus.loaded(obj)
    assert found is not None
    mapped = corpus.mapped(obj)
    assert not isinstance(mapped, str), mapped


def test_the_code_begins_where_the_runtime_says_it_does(obj: Path) -> None:
    # MODULE_CODE in the QuickBASIC 4.5 runtime's addr.inc is 48 bytes and the
    # offset past it is O_ENT; rtinit.asm calls that the beginning of the
    # user's code. The signature word is the first field, so the layout can be
    # checked rather than assumed.
    found = corpus.loaded(obj)
    assert found is not None
    assert has_header(found), "every object BC wrote carries a module header"
    mapped = corpus.mapped(obj)
    assert not isinstance(mapped, str)
    assert min(mapped.starts) == ENTRY


def test_only_an_event_build_has_a_stub_and_it_sits_after_the_jump(obj: Path) -> None:
    # Under /V or /W, PDS and VBDOS open the module with a jump over a
    # sixteen-byte event-poll routine that only the runtime enters. QuickBASIC
    # 4.5 sets the same U_FLAG bits and emits no stub.
    found = corpus.loaded(obj)
    assert found is not None
    stub = event_stub(found)
    mapped = corpus.mapped(obj)
    assert not isinstance(mapped, str)
    if stub is None:
        assert found.code[ENTRY : ENTRY + 2] != b"\xeb\x10"
    else:
        assert stub == ENTRY + 2
        assert stub in mapped.starts, "seeded, or nothing reaches it"


@pytest.mark.parametrize("tag", ["p-evt", "v-evt"])
def test_rebuilt_event_code_is_fully_visible(tag: str) -> None:
    """Rebuilt ADDRM hid 0035..0046 from the assembly dump and target scorer."""
    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.frontend.blocks import instructions
    result = wholeseg.emitted(Path(f"fixtures/omf/addrm-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR
    found = module.of(omf.parse(result.data))
    code = instructions(found)
    assert not isinstance(code, str), code
    assert event_stub(found) == 0x35
    assert 0x35 in {one.at for one in code}


def test_the_instruction_stream_tiles(mapped_obj: Path) -> None:
    # Overlapping instructions are what a decode that began one byte off looks
    # like, and its invented fragments are dangerous: `78 56`, the middle of the
    # constant 0x12345678, decodes as `js`, and retargeting that displacement
    # rewrites the constant.
    found = corpus.loaded(mapped_obj)
    assert found is not None
    reached = corpus.reached(mapped_obj)
    assert not isinstance(reached, str), reached
    for earlier, later in zip(reached, reached[1:], strict=False):
        assert earlier.end <= later.at


def test_an_on_goto_table_is_found_and_is_not_code(fixtures: Path) -> None:
    # BC compiles ON GOTO to a call to B$OGTA followed by inline data: a count
    # byte, then that many offset16 words. Reachability must step over them.
    found = corpus.loaded(fixtures / "jumptable.obj")
    assert found is not None
    mapped = corpus.mapped(fixtures / "jumptable.obj")
    assert not isinstance(mapped, str)
    assert mapped.tables == ((0x3F, 0x46), (0xEA, 0xEC))
    lo, hi = mapped.tables[0]
    assert found.code[lo] == 3, "the count byte says three labels"
    assert not [at for at in mapped.starts if lo <= at < hi], "no instruction begins inside a table"
    assert sorted(found.targets) == [0x46, 0x52, 0x5E, 0xEA]


def test_on_goto_comes_back_past_the_table(fixtures: Path) -> None:
    # ON 0 GOTO runs the next statement, so the byte after the table is a leader.
    found = corpus.loaded(fixtures / "jumps-v-g3.obj")
    assert found is not None
    mapped = corpus.mapped(fixtures / "jumps-v-g3.obj")
    assert not isinstance(mapped, str)
    assert mapped.tables
    from qbopt.frontend.blocks import statement_table
    inline = [table for table in mapped.tables if table != statement_table(found)]
    assert inline
    assert all(hi in mapped.leaders for _lo, hi in inline)


def test_padding_is_inert_but_a_branch_is_not(fixtures: Path) -> None:
    found = corpus.loaded(fixtures / "arith-v-g3.obj")
    assert found is not None
    assert benign(found, (0, 0)) == []
    padded = module.Module(found.records, found.seg, found.name, bytes([PAD, PAD]), 0, 2)
    assert benign(padded, (0, 2)) == []
    with_branch = module.Module(found.records, found.seg, found.name, hx("EB 00"), 0, 2)
    assert benign(with_branch, (0, 2)) is None
