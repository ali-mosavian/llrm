"""`uv run --project tools --with pytest python -m pytest tools/test_sizes.py`

sizes.py's instrument: it counts code, not the records the linker consumes (rule 3)."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from sizes import code_bytes  # noqa: E402


def record(kind: int, body: bytes) -> bytes:
    return bytes([kind]) + (len(body) + 1).to_bytes(2, "little") + body + b"\0"


def name(text: str) -> bytes:
    return bytes([len(text)]) + text.encode()


def object_with(code: int, extra: bytes = b"") -> bytes:
    """THEADR, LNAMES (_TEXT, CODE, _DATA, DATA), a code and a data SEGDEF, then `extra`."""
    lnames = name("_TEXT") + name("CODE") + name("_DATA") + name("DATA")
    segment = lambda size, at: bytes([0x48]) + size.to_bytes(2, "little") + bytes([at, at + 1, 1])  # noqa: E731
    return record(0x80, name("x.c")) + record(0x96, lnames) + record(0x98, segment(code, 1)) + record(0x98, segment(40, 3)) + extra + record(0x8A, b"\0")


def test_the_code_segment_is_counted_and_the_data_segment_is_not(tmp_path):
    path = tmp_path / "a.obj"
    path.write_bytes(object_with(123))
    assert code_bytes(path) == 123


def test_a_basic_objects_code_class_counts(tmp_path):
    """The QB frontend's objects name the class BC_CODE: they read 0 bytes while only CODE counted."""
    path = tmp_path / "b.obj"
    lnames = name("") + name("BC_CODE") + name("DIVMOD_CODE") + name("DATA")
    segment = bytes([0x48]) + (77).to_bytes(2, "little") + bytes([3, 2, 1])
    path.write_bytes(record(0x80, name("x")) + record(0x96, lnames) + record(0x98, segment))
    assert code_bytes(path) == 77


def test_a_change_to_fixups_alone_reads_zero_bytes(tmp_path):
    """sizes.py read the file's size: an inlined body naming a global once more grew lru.c by 78 bytes of
    FIXUPP records, with 18 more bytes of code."""
    plain, fixed = tmp_path / "plain.obj", tmp_path / "fixed.obj"
    plain.write_bytes(object_with(100))
    fixed.write_bytes(object_with(100, record(0x9C, b"\xc4\x01\x01\x54\x01" * 20)))
    assert fixed.stat().st_size > plain.stat().st_size
    assert code_bytes(fixed) - code_bytes(plain) == 0


def test_a_class_index_past_127_takes_two_bytes(tmp_path):
    """OMF indices from 0x80 up are two bytes: with 130 names the class index read one byte and the
    overlay index landed in its place."""
    path = tmp_path / "w.obj"
    lnames = b"".join(name(f"N{n}") for n in range(128)) + name("BC_CODE") + name("SEG")
    # class BC_CODE is name 129 (0x81 0x00 | 1: high bit set, then the low byte), segment SEG name 130.
    segment = bytes([0x48]) + (55).to_bytes(2, "little") + bytes([0x80, 130, 0x80, 129, 1])
    path.write_bytes(record(0x80, name("x")) + record(0x96, lnames) + record(0x98, segment))
    assert code_bytes(path) == 55
