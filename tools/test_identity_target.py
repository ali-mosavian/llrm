"""identity.sh must compile for the target its -m flag names, and refuse a flag it cannot map."""

import os
import subprocess
import tempfile
import unittest

TOOLS = os.path.dirname(os.path.abspath(__file__))


class IdentityTarget(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.d = self.dir.name
        self.log = os.path.join(self.d, "targets")
        for side in ("base", "new"):
            os.makedirs(os.path.join(self.d, side))
            cc = os.path.join(self.d, side, "llrm-cc")
            open(cc, "w").write(
                f'#!/bin/bash\n# {side}\necho "$LLRM_CC_TARGET" >> "$TARGET_LOG"\n'
                'while [ $# -gt 0 ]; do [ "$1" = -o ] && echo obj > "$2"; shift; done\n'
            )
            os.chmod(cc, 0o755)
            llc = os.path.join(self.d, side, "llrm-llc")
            open(llc, "w").write(f"#!/bin/bash\n# {side} llc\n" + '[ "$1" = --version ] && { echo "llrm-llc aaaa1111"; exit 0; }\n')
            os.chmod(llc, 0o755)

    def tearDown(self):
        self.dir.cleanup()

    def identity(self, *flag, same_llc="1"):
        return subprocess.run(
            [os.path.join(TOOLS, "identity.sh"), "c", os.path.join(self.d, "base", "llrm-cc"), os.path.join(self.d, "new", "llrm-cc"), *flag],
            capture_output=True, text=True, cwd=self.d,
            env={**os.environ, "LLRM_EXPECT_BASE_SHA": "aaaa1111", "LLRM_EXPECT_SHA": "aaaa1111", "IDENTITY_SAME_LLC": same_llc, "TARGET_LOG": self.log, "AB_ALLOW_ENV": "TARGET_LOG"},
        )

    def targets(self):
        return set(open(self.log).read().split())

    def test_m64_compiles_for_x86_64(self):
        """`-m64` fell through to the m16 default: every "m64 same" claim was an m16 -O2 result."""
        done = self.identity("-m64")
        self.assertEqual(self.targets(), {"x86_64-linux"}, done.stdout + done.stderr)

    def test_m32_and_the_default_keep_their_targets(self):
        self.identity("-m32")
        self.assertEqual(self.targets(), {"x86_32-wat"})
        os.remove(self.log)
        self.identity()
        self.assertEqual(self.targets(), {"x86_16-wat"})

    def test_an_unmapped_flag_is_refused_and_named(self):
        for flag in ("-m99", "-mfoo"):
            done = self.identity(flag)
            self.assertEqual(done.returncode, 2, done.stdout + done.stderr)
            self.assertIn(flag, done.stderr)
            self.assertFalse(os.path.exists(self.log), "nothing compiles for a target that cannot be named")

    def test_a_row_name_compiles_for_that_row(self):
        """Only -m16/-m32/-m64 were accepted: a dos, bcc or m68k identity run had no way to name its target."""
        done = self.identity("x86_32-dos")
        self.assertEqual(self.targets(), {"x86_32-dos"}, done.stdout + done.stderr)

    def test_an_unknown_row_is_refused(self):
        done = self.identity("x86_99-nowhere")
        self.assertEqual(done.returncode, 2, done.stdout + done.stderr)
        self.assertIn("x86_99-nowhere", done.stderr)
        self.assertFalse(os.path.exists(self.log))

    def test_a_row_whose_own_backend_is_shared_is_refused(self):
        """Only llrm-llc was compared: two dists sharing llrm-m68k-llc read SAME on every m68k row whatever changed."""
        for side in ("base", "new"):
            llc = os.path.join(self.d, side, "llrm-m68k-llc")
            open(llc, "w").write("#!/bin/bash\n# one m68k llc\n")
            os.chmod(llc, 0o755)
        done = self.identity("m68k-68000", same_llc="")
        self.assertEqual(done.returncode, 2, done.stdout + done.stderr)
        self.assertIn("same llrm-m68k-llc", done.stderr)
        self.assertFalse(os.path.exists(self.log))

    def test_the_run_says_its_target_and_what_it_did_not_test(self):
        done = self.identity("-m64")
        self.assertIn("target x86_64-linux", done.stdout)
        self.assertIn("NOT tested", done.stdout)


if __name__ == "__main__":
    unittest.main()
