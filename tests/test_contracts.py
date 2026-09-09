"""Dependency-aware contract inspection must not turn missing evidence into an ABI."""

import pytest
from libdump import LIBS
from libdump import Module
from libdump import modules
from contracts import cycles
from contracts import Library
from contracts import Contract
from contracts import summarize

from qbopt.objectfile import omf


def library(code: str) -> Library:
    payload = bytes.fromhex(code)
    return Library([Module("sample", [omf.Record(0xA0, b"\x01\x00\x00" + payload)])])


def contract(code: str) -> Contract:
    found = library(code)
    return summarize(found.graph((0, 1, 0)))[0, 1, 0]


def test_save_restore_across_dependency() -> None:
    """A caller saving AX around a clobbering helper must retain its original AX."""
    found = library("50 e80200 58 c3 31c0 c3")
    graph = found.graph((0, 1, 0))
    result = summarize(graph)
    assert len(result) == 2
    assert "ax" in result[0, 1, 6].clobbers
    assert "ax" in result[0, 1, 0].restored
    assert not result[0, 1, 0].unknown


def test_branch_beyond_first_return() -> None:
    """Stopping at the first RET missed the second path's AX clobber."""
    result = contract("7401 c3 31c0 c3")
    assert "ax" in result.clobbers


def test_memory_store_can_destroy_saved_value() -> None:
    """PUSH AX / store through BX / POP AX is not a proof when BX may alias SS:SP."""
    assert "ax" not in contract("50 8907 58 c3").preserved


@pytest.mark.parametrize("code", ["50 ffd3 58 c3", "50 8ed3 58 c3", "50 83c402 c3", "e8fdff c3"])
def test_unknown_paths_suppress_guarantees(code: str) -> None:
    result = contract(code)
    assert result.unknown
    assert not result.preserved


def test_transitive_clobber() -> None:
    """AX clobbered two helpers down must appear in the root contract."""
    found = library("e80100 c3 e80100 c3 31c0 c3")
    result = summarize(found.graph((0, 1, 0)))
    assert len(result) == 3
    assert all("ax" in one.clobbers for one in result.values())


@pytest.mark.parametrize("code", ["31c0 c3", "e80100 c3 31c0 c3"])
def test_arithmetic_preserves_direction(code: str) -> None:
    """Aggregate flags hid DF preservation needed to analyze FPDEEP's MOVSW copies."""
    result = contract(code)
    assert "df" in result.preserved
    assert "zf" in result.clobbers
    assert "flags" not in result.preserved


@pytest.mark.parametrize("code", ["fc c3", "fd c3", "e80100 c3 fd c3"])
def test_direction_write_is_not_entry_preservation(code: str) -> None:
    """CLD establishes a direction, but does not preserve an unknown incoming DF."""
    result = contract(code)
    assert "df" in result.clobbers
    assert "cf" in result.preserved


def test_string_copy_reads_direction() -> None:
    result = contract("a5 c3")
    assert "df" in result.reads
    assert "df" in result.preserved


@pytest.mark.parametrize("code", ["fc c3", "fd fc c3", "e80100 c3 fc c3", "fc e80100 c3 31c0 c3"])
def test_returned_direction_is_clear(code: str) -> None:
    """FPDEEP needs DF=0, not merely the fact that a helper preserves its input."""
    result = contract(code)
    assert result.flag_values["df"] == 0
    assert "df" not in result.preserved


def test_returned_direction_is_set() -> None:
    assert contract("fd c3").flag_values["df"] == 1


@pytest.mark.parametrize("code", ["c3", "7402 fc c3 fd c3", "fc ffd3 c3", "fc e8fcff c3"])
def test_direction_requires_complete_agreeing_returns(code: str) -> None:
    """Disagreeing exits and unresolved dependencies cannot promise forward copies."""
    assert "df" not in contract(code).flag_values


def test_cleanup_propagates() -> None:
    result = contract("50 e80100 c3 c20200")
    assert result.cleanup == 0
    assert not result.unknown


def test_low_byte_write_preserves_other_lanes() -> None:
    """MOV AL,1 must not claim AH is clobbered or AX/EAX preserved."""
    result = contract("b001 c3")
    assert {"al", "ax", "eax"} <= set(result.clobbers)
    assert {"ah", "eax[31:16]"} <= set(result.preserved)


def test_word_write_preserves_upper_half() -> None:
    result = contract("b80100 c3")
    assert {"al", "ah", "ax", "eax"} <= set(result.clobbers)
    assert "eax[31:16]" in result.preserved


def test_dword_save_restores_every_lane() -> None:
    result = contract("6650 66b801000000 6658 c3")
    assert {"al", "ah", "ax", "eax", "eax[31:16]"} <= set(result.restored)
    assert not result.unknown


def test_word_save_does_not_restore_upper_half() -> None:
    result = contract("50 66b801000000 58 c3")
    assert "ax" in result.restored
    assert "eax" in result.clobbers
    assert "eax[31:16]" in result.clobbers


def test_partial_callee_effect_keeps_lane_precision() -> None:
    result = contract("e80100 c3 b401 c3")
    assert {"ah", "ax", "eax"} <= set(result.clobbers)
    assert {"al", "eax[31:16]"} <= set(result.preserved)


def test_multiple_roots_share_one_helper() -> None:
    found = library("e80500 c3 e80100 c3 b001 c3")
    graph = found.graph_from([(0, 1, 0), (0, 1, 4)])
    result = summarize(graph)
    assert len(graph) == 3
    assert all("al" in one.clobbers and "ah" in one.preserved for one in result.values())


def test_code_roots_exclude_data_symbols() -> None:
    found = library("c3")
    found.objects[0].records.extend(
        [
            omf.Record(omf.LNAMES, b"\x04text\x04CODE\x04data\x04DATA"),
            omf.Record(omf.SEGDEF, b"\x20\x01\x00\x01\x02\x00"),
            omf.Record(omf.SEGDEF, b"\x20\x01\x00\x03\x04\x00"),
        ]
    )
    found.names = {(0, 1, 0): "function", (0, 2, 0): "variable"}
    assert found.code_entries({"CODE"}) == [(0, 1, 0)]


def test_constant_branch_excludes_unreachable_call() -> None:
    """Like ETS2's DL=2: JNE skips a helper that cannot run at this entry."""
    found = library("b202 80fa00 7503 e80100 c3 31c0 c3")
    graph = found.graph((0, 1, 0))
    assert len(graph) == 1
    assert not graph[0, 1, 0].calls


def test_real_ets2_excludes_trigger_read() -> None:
    """ETS2's constant DL=2 excludes RDTRIG; the old report listed it as a dependency."""
    path = LIBS["vbdos"]
    if not path.is_file():
        pytest.skip("VBDOS runtime library unavailable")
    found = Library(modules(path.read_bytes()))
    (root,) = found.symbols["B$ETS2"]
    routine = found.decode(root)
    assert 0x42 not in routine.calls
    assert 0x49 in routine.calls  # EVNT_SET is still called.


def test_join_discards_conflicting_byte_constants() -> None:
    """Unknown incoming ZF selects DL=0 or DL=2; neither later edge is impossible."""
    found = library("7404 b200 eb02 b202 80fa00 7503 e80100 c3 c3")
    routine = found.decode((0, 1, 0))
    assert 13 in routine.calls


def test_call_discards_known_register_values() -> None:
    found = library("b202 e80900 80fa00 7503 e80100 c3 c3")
    routine = found.decode((0, 1, 0))
    assert 10 in routine.calls


def test_relocated_immediate_is_not_a_literal(monkeypatch: pytest.MonkeyPatch) -> None:
    import contracts

    original = contracts.constant_paths
    monkeypatch.setattr(contracts, "constant_paths", lambda routine, relocated: routine)
    found = library("b202 80fa00 7503 e80100 c3 c3")
    changed = original(found.decode((0, 1, 0)), {0})
    assert 7 in changed.calls


def test_partial_write_invalidates_only_its_own_byte() -> None:
    # DL remains two even though DH is overwritten from unknown AH.
    found = library("b202 8af4 80fa00 7503 e80100 c3 c3")
    assert not found.decode((0, 1, 0)).calls


def test_branch_to_next_instruction_keeps_the_return() -> None:
    result = contract("31c0 7400 c3")
    assert result.cleanup == 0
    assert not result.unknown


def test_report_groups_register_aliases_without_losing_effects() -> None:
    from contracts import contract_report

    original = contract("b001 b501 c3")
    report = contract_report(original)
    assert report["clobbers"] == {"eax": ["eax", "ax", "al"], "ecx": ["ecx", "cx", "ch"]}
    for field in ("reads", "clobbers", "preserved", "restored"):
        assert {name for group in report[field].values() for name in group} == set(getattr(original, field))
    assert report["preserved"]["eax"] == ["ah", "eax[31:16]"]


def test_budget_is_unknown() -> None:
    found = library("e80100 c3 31c0 c3")
    found.functions = 1
    result = summarize(found.graph((0, 1, 0)))[0, 1, 0]
    assert result.unknown
    assert not result.preserved


def test_cycle_is_separate_from_caller() -> None:
    """A caller of a recursive function is not itself a recursion cycle."""
    found = library("e80100 c3 e8fdff c3")
    graph = found.graph((0, 1, 0))
    assert cycles(graph) == [{(0, 1, 4)}]
    assert all(result.unknown for result in summarize(graph).values())


def test_library_page_size() -> None:
    """The hard-coded 16-byte page reader stopped before the second module in a 64-byte-page LIB."""

    def record(kind: int, body: bytes) -> bytes:
        header = bytes([kind]) + (len(body) + 1).to_bytes(2, "little") + body
        return header + bytes([-sum(header) & 255])

    header = record(0xF0, bytes(60))
    first = record(0x80, b"\x01a") + record(0x8A, b"\x00")
    second = record(0x80, b"\x01b") + record(0x8A, b"\x00")
    data = header + first + bytes(64 - len(first)) + second
    assert [module.name for module in modules(data)] == ["a", "b"]


def test_real_library_dependencies() -> None:
    """FreeHandleBlock calls local 0xda then FreeHandle; raw E8 placeholders point elsewhere."""
    path = LIBS["vbdos"]
    if not path.is_file():
        pytest.skip("VBDOS runtime library unavailable")
    found = Library(modules(path.read_bytes()))
    (root,) = found.symbols["B$FreeHandleBlock"]
    graph = found.graph(root)
    assert len(graph) == 3
    assert any(routine.address[2] == 0xDA for routine in graph.values())
    assert any(routine.name.endswith(":B$FreeHandle") for routine in graph.values())
    result = summarize(graph)[root]
    assert result.cleanup == 4
    assert "cx" in result.clobbers
    assert "ax" in result.preserved


def test_object_fixture_is_readable() -> None:
    from pathlib import Path

    found = Library(modules(Path("fixtures/omf/procs-v-g3.obj").read_bytes()))
    assert found.symbols
    root = next(iter(found.symbols.values()))[0]
    result = summarize(found.graph(root))[root]
    assert result.unknown  # Runtime dependencies were not supplied.
