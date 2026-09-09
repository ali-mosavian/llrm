"""Dependency-aware contract inspection must not turn missing evidence into an ABI."""

import pytest
from libdump import LIBS
from libdump import Module
from libdump import modules
from contracts import cycles
from contracts import Library
from contracts import Contract
from contracts import summarize

from qbopt import omf


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


def test_cleanup_propagates() -> None:
    result = contract("50 e80100 c3 c20200")
    assert result.cleanup == 0
    assert not result.unknown


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
