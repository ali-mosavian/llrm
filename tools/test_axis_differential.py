"""`uv run --project tools --with pytest python -m pytest tools/test_axis_differential.py`

The comparison itself must be able to fail (rule 2): a data axis that disagrees with the image is reported."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import axis_differential as ad  # noqa: E402
import sizes  # noqa: E402
from test_sizes import lnames, mz, record, segdef, stored_data, name  # noqa: E402


def test_cells_that_grow_the_image_and_the_axis_alike_pass():
    assert ad.differences((100, 0), {"a": (116, 16), "b": (100, 0), "c": (1100, 1000)}) == []


def test_an_axis_that_misses_stored_bytes_is_reported():
    """The class rule counted a BSS jwlink stored before the stack as not stored: the image grew 30 B, the axis 0."""
    assert ad.differences((100, 0), {"bss": (130, 0)}) == ["bss: image +30, axis +0"]


def test_the_comparison_reads_the_header_not_the_file_length(tmp_path):
    exe = tmp_path / "t.exe"
    exe.write_bytes(mz(40) + b"trailing overlay")
    assert ad.image(exe) == 40


def test_the_class_rule_fails_it_where_the_image_stores_a_bss(tmp_path):
    """The two data-axis sources on the 49-exe shape: the class rule (no image) says 10 stored, the image holds 40."""
    obj = tmp_path / "n.obj"
    obj.write_bytes(record(0x80, name("x")) + lnames("_DATA", "DATA", "_BSS", "BSS") + segdef(10, 1, 2) + segdef(30, 3, 4) + stored_data(1, bytes([1]) * 10))
    listing = tmp_path / "n.map"
    listing.write_text("\n Start  Stop   Length Name                   Class\n 00000H 00009H 0000AH _DATA                  DATA\n 0000AH 00027H 0001EH _BSS                   BSS\n 00030H 000FFH 000D0H STACK                  STACK\n")
    exe = tmp_path / "s.exe"
    exe.write_bytes(mz(40))
    base = (10, 10)
    assert ad.differences(base, {"class rule": (ad.image(exe), sizes.data_bytes(obj)[0])}) == ["class rule: image +30, axis +0"]
    assert ad.differences(base, {"image rule": (ad.image(exe), sizes.data_bytes(obj, exe, listing)[0])}) == []
