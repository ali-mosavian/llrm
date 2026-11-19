"""The corpus cells read os.path.getsize of an sdas `.rel` as the object's size: 1030 'bytes' for +176 of code on
z80 known_loads_walks O2. The code is the `A _CODE size` line."""
import os
import tempfile

from rel_code_size import rel_code_size

REL = "XH3\nH 2 areas 0 global symbols\nA _CODE size 1A flags 0 addr 0\nA _DATA size 4 flags 0 addr 0\nT 00 00 3E 01 C9\nR 00 00 00 00\n"


def test_the_size_is_the_code_area_not_the_file_length():
    with tempfile.NamedTemporaryFile("w", suffix=".rel", delete=False) as one:
        one.write(REL)
    try:
        assert rel_code_size(one.name) == 0x1A
        assert os.path.getsize(one.name) != 0x1A
    finally:
        os.unlink(one.name)
