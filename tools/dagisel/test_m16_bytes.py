"""m16_bytes.py counts what the compilers emit."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import m16_bytes as tool  # noqa: E402


def test_pin_text_is_the_text_section_not_the_total():
    assert tool.pin_text("section   size  addr\n.text      419     0\n.data       4     0\nTotal     425\n") == 419


def test_emitted_counts_ledata_data_bytes_only(tmp_path):
    ledata = bytes([0xA0, 7, 0, 1, 0, 0, 0xAA, 0xBB, 0xCC, 0])  # segment 1, offset 0, three data bytes, checksum
    path = tmp_path / "o.obj"
    path.write_bytes(bytes([0x80, 2, 0, 0, 0]) + ledata)  # a THEADR, then the LEDATA
    assert tool.emitted(path) == 3
