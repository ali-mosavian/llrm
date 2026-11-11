#!/usr/bin/env python3
"""ours.py measured the NMOS 6502 only: the 65C02 A/B of #8949 could not be run (2026-11-10). OURS_CPU picks the target;
an unknown value must be refused, not measured as a 6502."""
import os, subprocess, sys, tempfile, unittest

HERE = os.path.dirname(os.path.abspath(__file__))


class OursCpu(unittest.TestCase):
    def test_an_unknown_cpu_is_refused(self):
        with tempfile.NamedTemporaryFile(suffix=".s") as rt:
            env = {**os.environ, "OURS_RT": rt.name, "OURS_CPU": "z80"}
            r = subprocess.run([sys.executable, os.path.join(HERE, "ours.py"), "/nonexistent/llrm-cc", "O2", "/dev/null", "__empty__"],
                               capture_output=True, text=True, env=env, timeout=120)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("OURS_CPU", r.stderr + r.stdout)


if __name__ == "__main__":
    unittest.main()
