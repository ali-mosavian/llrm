"""The A/B drivers refuse a pipeline-changing LLRM_* switch before doing any work."""

import os
import subprocess
import unittest

TOOLS = os.path.dirname(__file__)
DRIVERS = ["identity.sh", "ab/cells.sh", "wasm_identity.sh", "wasm_corpus.sh", "llc_backend_diff.sh", "llc_check.sh", "cc_check.sh"]


def run(script, **extra):
    env = {k: v for k, v in os.environ.items() if not k.startswith(("LLRM_", "AB_ALLOW"))}
    env.update(extra)
    return subprocess.run(["bash", os.path.join(TOOLS, script), "x", "y", "z"], env=env, capture_output=True, text=True, cwd="/")


class DriversRefuseSwitches(unittest.TestCase):
    def test_every_driver_refuses_a_backend_only_switch(self):
        """LLRM_WASM_BACKEND_ONLY=1 skipped the mid-end on one side; a second cause was chased for 40 minutes."""
        for script in DRIVERS:
            done = run(script, LLRM_WASM_BACKEND_ONLY="1")
            self.assertEqual(done.returncode, 2, (script, done.stderr))
            self.assertIn("LLRM_WASM_BACKEND_ONLY", done.stderr, script)

    def test_a_value_with_spaces_is_kept_whole(self):
        done = run("identity.sh", LLRM_NO_COMBINE="a b")
        self.assertIn("LLRM_NO_COMBINE=a b", done.stderr)


if __name__ == "__main__":
    unittest.main()
