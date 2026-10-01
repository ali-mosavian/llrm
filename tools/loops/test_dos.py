"""`uv run --project tools python -m unittest discover -s tools/loops`"""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dos  # noqa: E402
from build import CompileError  # noqa: E402


def exe(image: int) -> bytes:
    """An MZ header for `image` bytes after a 32-paragraph header, then the bytes."""
    total = 512 + image
    head = b"MZ" + (total % 512).to_bytes(2, "little") + ((total + 511) // 512).to_bytes(2, "little")
    head += (0).to_bytes(2, "little") + (32).to_bytes(2, "little") + (0).to_bytes(2, "little") + (65535).to_bytes(2, "little")
    return head + bytes(498 - 0) + bytes(image)


class LoadTests(unittest.TestCase):
    def test_a_batch_too_big_for_dos_memory_is_refused(self):
        """A 755K batch EXE (the quick set once huge globals built) failed on DOS
        with errcode=8 and counted as a wrong result of the compiler."""
        with tempfile.TemporaryDirectory() as where:
            big, small = Path(where) / "BIG.EXE", Path(where) / "SMALL.EXE"
            big.write_bytes(exe(755_000))
            small.write_bytes(exe(100_000))
            dos.check_loads(small)
            with self.assertRaises(CompileError):
                dos.check_loads(big)


if __name__ == "__main__":
    unittest.main()
