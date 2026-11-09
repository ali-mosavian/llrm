"""A deviation that is a number in a row, not a comment: the frame's rounding names its row, and the row names owner, review date and price."""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def test_the_frame_rounding_deviation_names_its_row_and_the_row_has_owner_review_and_price():
    frame = (ROOT / "crates/target/llrm-m68k/src/frame.rs").read_text()
    assert "DEVIATE M68K-STACK-ALIGN" in frame
    row = (ROOT / "docs/working/m68k-stack-align-row.md").read_text()
    assert "Owner: m68k" in row and "Review: 2026-12-20" in row
    assert "| 68000 | O2 |" in row and "| 68020 | Os |" in row, "the byte price per cpu and level"
