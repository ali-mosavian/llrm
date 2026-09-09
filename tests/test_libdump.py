"""Partial runtime listings must not masquerade as whole-function evidence."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import libdump


def test_event_fast_return_reports_the_unvisited_pending_branch():
    """B$EVK1's CMP/JNE/RETF listing omitted its pending-event dispatch path."""
    code = bytes.fromhex("833e0000007501cb90cb")
    listing = libdump.disassemble(code, 0)
    assert any("unvisited branch targets: 0008" in line for line in listing)
    assert any("contracts.py" in line for line in listing)


def test_backedge_inside_listing_does_not_report_a_missing_target():
    listing = libdump.disassemble(bytes.fromhex("4975fdc3"), 0)
    assert not any("unvisited branch" in line for line in listing)
