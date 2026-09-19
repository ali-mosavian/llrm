from pathlib import Path

import pytest
from qbfootprint import MapLinkError
from qbfootprint import compare_maps
from qbfootprint import linked_code_totals


def test_footprint_counts_linked_code_segments_not_object_files(tmp_path: Path) -> None:
    """The QB frontend ledger must price linked BC_CODE, not OMF container bytes."""
    bc = tmp_path / "bc.map"
    qbopt = tmp_path / "qbopt.map"
    bc.write_text(
        " 00000H 0001FH 00020H MAIN_CODE BC_CODE\n"
        " 00020H 0002FH 00010H AUX_CODE BC_CODE\n"
        " 00030H 0012FH 00100H DATA BC_DATA\n"
    )
    qbopt.write_text(
        " 00000H 00017H 00018H MAIN_CODE BC_CODE\n"
        " 00020H 00037H 00018H AUX_CODE BC_CODE\n"
        " 00040H 0023FH 00200H DATA BC_DATA\n"
    )

    rows, totals = compare_maps(bc, qbopt)

    assert rows == [("AUX", 16, 24), ("MAIN", 32, 24)]
    assert totals == (48, 48)


def test_footprint_prices_helpers_pulled_in_by_basic_calls(tmp_path: Path) -> None:
    """The old BC_CODE-only ledger hid runtime helpers behind five-byte calls."""
    bc = tmp_path / "bc.map"
    qbopt = tmp_path / "qbopt.map"
    bc.write_text(
        " 00000H 0001FH 00020H MAIN_CODE BC_CODE\n"
        " 00020H 0004FH 00030H QB_RUNTIME CODE\n"
        " 00050H 0014FH 00100H DATA BC_DATA\n"
    )
    qbopt.write_text(
        " 00000H 0002FH 00030H MAIN_CODE BC_CODE\n"
        " 00030H 0003FH 00010H QB_RUNTIME CODE\n"
        " 00040H 0023FH 00200H DATA BC_DATA\n"
    )

    assert linked_code_totals(bc, qbopt) == (80, 64)


def test_footprint_rejects_a_map_from_a_failed_link(tmp_path: Path) -> None:
    """The first qrender baseline survived unresolved externals and understated code."""
    failed = tmp_path / "failed.map"
    good = tmp_path / "good.map"
    failed.write_text(
        " 00000H 0001FH 00020H MAIN_CODE BC_CODE\nmain.obj : error L2029: 'MISSING' : unresolved external\n"
    )
    good.write_text(" 00000H 0001FH 00020H MAIN_CODE BC_CODE\n")

    with pytest.raises(MapLinkError, match="unresolved external"):
        compare_maps(failed, good)
