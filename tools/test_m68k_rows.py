"""A deviation that is a number in a row, not a comment: the frame's rounding names its row, and the row names owner, review date and price."""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def test_the_frame_rounding_deviation_names_its_row_and_the_row_has_owner_review_and_price():
    frame = (ROOT / "crates/target/llrm-m68k/src/frame.rs").read_text()
    assert "DEVIATE M68K-STACK-ALIGN" in frame
    row = (ROOT / "docs/working/m68k-stack-align-row.md").read_text()
    assert "Owner: m68k" in row and "Review: 2026-12-20" in row
    assert "| 68000 | O2 |" in row and "| 68020 | Os |" in row, "the byte price per cpu and level"


def test_the_known_wrong_align_row_names_owner_exit_and_its_executed_evidence():
    """An over-aligned local was silently misaligned at 12 of 16 depths; the row carries the harness that shows it."""
    row = (ROOT / "docs/working/m68k-known-wrong-align.md").read_text()
    assert "M68K-KNOWN-WRONG-ALIGN" in row and "Owner: m68k" in row and "Exit:" in row
    for name in ("aligned16_local.c", "aligned16_main.c"):
        assert name in row and (ROOT / "crates/target/llrm-m68k/tests/data_rt" / name).exists(), name


def test_the_known_wrong_align_row_exits_on_the_shared_realign_date():
    row = (ROOT / "docs/working/m68k-known-wrong-align.md").read_text()
    assert "stack realign 2026-11-14" in row
