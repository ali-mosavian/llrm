import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import jpeg_reference as reference  # noqa: E402

BENCH = Path(__file__).resolve().parents[1] / "bench"
PHOTO = (BENCH / "jpeg_single" / "photo.jpg").read_bytes()


def expected(name: str) -> int:
    return int((BENCH / name / f"{name}.out").read_text())


def test_fixed_reference_is_the_checksum_the_benches_print():
    assert reference.adler(reference.run(PHOTO, True)) == expected("jpeg_fixed")


def test_single_reference_is_the_checksum_the_benches_print():
    assert reference.adler(reference.run(PHOTO, False)) == expected("jpeg_single")


def test_tmul_truncates_toward_zero():
    """A floored product would give -1 for -1/1024 and -2 for -1.5/1024."""
    assert [reference.tmul(a, b) for a, b in ((-1, 1), (-1536, 1), (1, -1536), (1536, 1))] == [0, -1, -1, 1]
