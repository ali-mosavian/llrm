"""
Which bytes are code, found by reachability rather than assumed.
"""

from pathlib import Path

import pytest

from helpers import hx
from qbopt import module
from qbopt.blocks import PAD
from qbopt.blocks import Ends
from qbopt.blocks import ENTRY
from qbopt.blocks import benign
from qbopt.declen import decode
from qbopt.blocks import code_map
from qbopt.blocks import event_stub
from qbopt.blocks import has_header
from qbopt.blocks import terminator
from qbopt.blocks import instructions


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


def test_every_module_is_mapped(obj: Path) -> None:
    found = module.load(obj)
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str), mapped


def test_the_code_begins_where_the_runtime_says_it_does(obj: Path) -> None:
    # MODULE_CODE in the QuickBASIC 4.5 runtime's addr.inc is 48 bytes and the
    # offset past it is O_ENT; rtinit.asm calls that the beginning of the
    # user's code. The signature word is the first field, so the layout can be
    # checked rather than assumed.
    found = module.load(obj)
    assert found is not None
    assert has_header(found), "every object BC wrote carries a module header"
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    assert min(mapped.starts) == ENTRY


def test_only_an_event_build_has_a_stub_and_it_sits_after_the_jump(obj: Path) -> None:
    # Under /V or /W, PDS and VBDOS open the module with a jump over a
    # sixteen-byte event-poll routine that only the runtime enters. QuickBASIC
    # 4.5 sets the same U_FLAG bits and emits no stub.
    found = module.load(obj)
    assert found is not None
    stub = event_stub(found)
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    if stub is None:
        assert found.code[ENTRY : ENTRY + 2] != b"\xeb\x10"
    else:
        assert stub == ENTRY + 2
        assert stub in mapped.starts, "seeded, or nothing reaches it"


def test_the_instruction_stream_tiles(mapped_obj: Path) -> None:
    # Overlapping instructions are what a decode that began one byte off looks
    # like, and its invented fragments are dangerous: `78 56`, the middle of the
    # constant 0x12345678, decodes as `js`, and retargeting that displacement
    # rewrites the constant.
    found = module.load(mapped_obj)
    assert found is not None
    reached = instructions(found)
    assert not isinstance(reached, str), reached
    for earlier, later in zip(reached, reached[1:], strict=False):
        assert earlier.end <= later.at


def test_an_on_goto_table_is_found_and_is_not_code(fixtures: Path) -> None:
    # BC compiles ON GOTO to a call to B$OGTA followed by inline data: a count
    # byte, then that many offset16 words. Reachability must step over them.
    found = module.load(fixtures / "jumptable.obj")
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    assert mapped.tables == ((0x3F, 0x46),)
    lo, hi = mapped.tables[0]
    assert found.code[lo] == 3, "the count byte says three labels"
    assert not [at for at in mapped.starts if lo <= at < hi], "no instruction begins inside a table"
    assert sorted(found.targets) == [0x46, 0x52, 0x5E, 0xEA]


def test_on_goto_comes_back_past_the_table(fixtures: Path) -> None:
    # ON 0 GOTO runs the next statement, so the byte after the table is a leader.
    found = module.load(fixtures / "jumps-v-g3.obj")
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    assert mapped.tables
    assert all(hi in mapped.leaders for _lo, hi in mapped.tables)


def test_padding_is_inert_but_a_branch_is_not(fixtures: Path) -> None:
    found = module.load(fixtures / "arith-v-g3.obj")
    assert found is not None
    assert benign(found, (0, 0)) == []
    padded = module.Module(found.records, found.seg, found.name, bytes([PAD, PAD]), 0, 2)
    assert benign(padded, (0, 2)) == []
    with_branch = module.Module(found.records, found.seg, found.name, hx("EB 00"), 0, 2)
    assert benign(with_branch, (0, 2)) is None
