"""A dist compiles after it moves, and two builds of one sha are the same bytes.

    python3 -m unittest tools/pairs/test_dist_relocatable.py

DIST_LLRM_CC names the llrm-cc to test (default: skipped). It is copied out of its
build tree and run under strace: reading anything from the tree it was built in
is the failure; a dist whose tree is gone gave E1055.
SCRATCH_A and SCRATCH_B, two pair_build.sh scratch roots at different paths,
and SHA enable the slow test: both dists of SHA must have one md5.
"""
import hashlib
import os
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

DIST = os.environ.get("DIST_LLRM_CC")
HAVE_STRACE = shutil.which("strace") is not None


class Relocatable(unittest.TestCase):
    @unittest.skipUnless(DIST and HAVE_STRACE, "DIST_LLRM_CC and strace")
    def test_a_dist_that_moved_reads_nothing_from_the_tree_it_was_built_in(self):
        """a dist opened files under its build tree: a removed tree failed every compile (E1055)."""
        built = Path(DIST).resolve()
        with tempfile.TemporaryDirectory() as tmp:
            moved = Path(tmp) / "moved" / "llrm-cc"
            moved.parent.mkdir()
            shutil.copy2(built, moved)
            (Path(tmp) / "t.c").write_text("int f(int a) { return a * 2; }\n")
            for flags, row in ((["-m16"], "x86_16-wat"), (["-m32", "-mabi=sysv"], "x86_32-wat")):
                log = Path(tmp) / "strace.txt"
                done = subprocess.run(
                    ["strace", "-f", "-e", "trace=openat,execve", "-o", str(log), str(moved), "-c", *flags, "-O2",
                     "-o", str(Path(tmp) / "t.obj"), str(Path(tmp) / "t.c")],
                    capture_output=True, text=True, env={**os.environ, "LLRM_CC_TARGET": row})
                self.assertEqual(done.returncode, 0, done.stderr)
                opened = re.findall(r'"([^"]+)"[^\n]*= \d+\n', log.read_text())
                # The tree is where the binary was built: its target dir's parent holds the worktree.
                tree = [path for path in opened if str(built.parents[2]) in path and path != str(moved)]
                self.assertEqual(tree, [], flags)

    @unittest.skipUnless(os.environ.get("SHA") and os.environ.get("SCRATCH_A") and os.environ.get("SCRATCH_B"), "SHA, SCRATCH_A, SCRATCH_B")
    def test_one_sha_built_in_two_trees_is_one_binary(self):
        """Absolute build paths in the binary made equal sources differ by md5 between worktrees."""
        sums = []
        for scratch in (os.environ["SCRATCH_A"], os.environ["SCRATCH_B"]):
            subprocess.run(["tools/pairs/pair_build.sh", scratch, os.environ["SHA"]], check=True)
            dist = next(Path(scratch).glob("sbp-*/dist/llrm-cc"))
            sums.append(hashlib.md5(dist.read_bytes()).hexdigest())
        self.assertEqual(sums[0], sums[1])


if __name__ == "__main__":
    unittest.main()
