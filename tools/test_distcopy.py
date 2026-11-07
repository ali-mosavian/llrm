"""Copying a binary into a dist dir must never write through a symlink there: a `cp` into a hybrid dist whose `llrm-nib` linked to
another dist replaced that dist's compiler (11-05, every number read from it for half an hour came from the wrong binary)."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import distcopy  # noqa: E402


class DistCopyTests(unittest.TestCase):
    def test_a_symlink_to_another_dist_is_refused_and_its_target_kept(self):
        with tempfile.TemporaryDirectory() as tmp:
            other, dist, source = Path(tmp, "other"), Path(tmp, "dist"), Path(tmp, "src", "llrm-nib")
            other.mkdir()
            source.parent.mkdir()
            dist.mkdir()
            (other / "llrm-nib").write_bytes(b"original")
            (dist / "llrm-nib").symlink_to(other / "llrm-nib")
            source.write_bytes(b"patched")
            with self.assertRaises(distcopy.Refused):
                distcopy.copy_into(source, dist)
            self.assertEqual((other / "llrm-nib").read_bytes(), b"original")

    def test_a_plain_destination_is_replaced_by_a_copy(self):
        with tempfile.TemporaryDirectory() as tmp:
            dist, source = Path(tmp, "dist"), Path(tmp, "src", "llrm-nib")
            dist.mkdir()
            source.parent.mkdir()
            (dist / "llrm-nib").write_bytes(b"old")
            source.write_bytes(b"new")
            distcopy.copy_into(source, dist)
            self.assertEqual((dist / "llrm-nib").read_bytes(), b"new")


if __name__ == "__main__":
    unittest.main()
