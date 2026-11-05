"""qcport-compile.sh with no modules compiled nothing and exited 0 ("0 modules, 0 refusals")."""

import os
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(ROOT, "tools", "qcport-compile.sh")


def run(modules, compiler_ok=True):
    with tempfile.TemporaryDirectory() as d:
        src = os.path.join(d, "src")
        for sub in ("host", "render", "model", "game", "sound", "ui"):
            os.makedirs(os.path.join(src, sub))
        for m in range(modules):
            open(os.path.join(src, "host", f"m{m}.c"), "w").close()
        fake = os.path.join(d, "llrm-c")
        with open(fake, "w") as f:
            f.write("#!/bin/sh\n" + ("exit 0\n" if compiler_ok else "echo 'llrm-c: refused'; exit 1\n"))
        os.chmod(fake, 0o755)
        env = dict(os.environ, QCPORT=src, QCPORT_INC=d)
        return subprocess.run(["bash", SCRIPT, fake], cwd=ROOT, env=env, capture_output=True, text=True)


class QcportCompileExit(unittest.TestCase):
    def test_no_modules_is_not_green(self):
        r = run(0)
        self.assertNotEqual(r.returncode, 0, r.stdout)

    def test_refusal_is_not_green(self):
        self.assertNotEqual(run(1, compiler_ok=False).returncode, 0)

    def test_compiled_is_green(self):
        r = run(1)
        self.assertEqual(r.returncode, 0, r.stdout)


if __name__ == "__main__":
    unittest.main()
