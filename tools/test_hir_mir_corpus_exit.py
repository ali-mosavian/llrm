"""hir-mir-corpus.sh with every frontend failing emitted nothing, only echoed 'frontend fails', and exited 0."""

import os
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(ROOT, "tools", "hir-mir-corpus.sh")


def stub(path, body):
    with open(path, "w") as f:
        f.write("#!/bin/sh\n" + body)
    os.chmod(path, 0o755)


def run(frontends_work):
    with tempfile.TemporaryDirectory() as d:
        bindir, llvm, fake = (os.path.join(d, n) for n in ("bin", "llvm", "fake"))
        for p in (bindir, llvm, fake):
            os.makedirs(p)
        stub(os.path.join(fake, "cargo"), "exit 0\n")  # no build
        stub(os.path.join(llvm, "opt"), "exit 0\n")
        # llrm-qb --dump-hir FILE writes the JSON; llrm-nib --dump DIR writes DIR/03-hir.json; hir-mir prints IR
        stub(os.path.join(bindir, "llrm-qb"), 'f=$3; ' + ('echo {} > "$f"; exit 0\n' if frontends_work else "exit 1\n"))
        stub(os.path.join(bindir, "llrm-nib"), ('mkdir -p "$3"; echo {} > "$3/03-hir.json"; exit 0\n' if frontends_work else "exit 1\n"))
        stub(os.path.join(bindir, "hir-mir"), "echo 'define void @f() {}'\n")
        env = dict(os.environ, LLRM_BIN=bindir, LLVM20=llvm, PATH=fake + os.pathsep + os.environ["PATH"])
        return subprocess.run(["bash", SCRIPT, os.path.join(d, "out")], cwd=ROOT, env=env, capture_output=True, text=True)


class HirMirCorpusExit(unittest.TestCase):
    def test_every_frontend_failing_is_not_green(self):
        r = run(False)
        self.assertNotEqual(r.returncode, 0, r.stdout[-300:])

    def test_working_frontends_are_green(self):
        r = run(True)
        self.assertEqual(r.returncode, 0, r.stdout[-300:] + r.stderr[-300:])


if __name__ == "__main__":
    unittest.main()
