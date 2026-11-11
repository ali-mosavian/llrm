"""cc-sweep.sh with a bin directory that lacks a target's backend: `arm64-linux: ok 0 fail 74` was the instrument saying nothing."""

import os
import stat
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
TARGETS = TOOLS.parent / "crates/driver/llrm-cc/targets.toml"


def stub(path, body="exit 1\n"):
    path.write_text("#!/bin/sh\n" + body)
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


class CcSweep(unittest.TestCase):
    def sweep(self, backends, target):
        with tempfile.TemporaryDirectory() as d:
            for name in ("llrm-cc", *backends):
                stub(Path(d) / name)
            return subprocess.run(
                ["sh", str(TOOLS / "cc-sweep.sh"), target],
                capture_output=True, text=True, timeout=120,
                env={**os.environ, "LLRM_BIN": d, "CC_SWEEP_OUT": f"{d}/out"},
            )

    def test_a_target_whose_backend_is_not_in_the_dist_is_refused_by_name(self):
        """The dist had llrm-cc and llrm-llc only: every arm64 file failed and the sweep printed `ok 0 fail 74`, rc 0."""
        done = self.sweep(["llrm-llc"], "arm64-linux")
        self.assertNotEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn("llrm-arm64-llc", done.stderr)

    def test_a_target_whose_backend_is_there_is_swept(self):
        """Control: with the backend present the guard stays out of the way (the stub compiler fails every file; the table still prints)."""
        done = self.sweep(["llrm-llc", "llrm-arm64-llc"], "arm64-linux")
        self.assertIn("arm64-linux: ok 0 fail", done.stdout, done.stdout + done.stderr)

    def test_every_backend_a_target_names_is_built_by_the_objects_dist(self):
        """A new target with its own llc must join EXTRA_LLCS, or the dist the sweeps run on lacks it."""
        import sys

        sys.path.insert(0, str(TOOLS))
        import measure

        rows = tomllib.loads(TARGETS.read_text())
        named = {row.get("llc_program", "llrm-llc") for row in rows.values()} - {"llrm-llc"}
        self.assertLessEqual(named, set(measure.EXTRA_LLCS))


if __name__ == "__main__":
    unittest.main()
