"""tools/arm64/cell.sh: a cell whose runtime calls could not be read is not a measured cell."""

import os
import shutil
import subprocess
import tempfile
import unittest

HERE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "arm64")

# The neighbours cell.sh calls, answering for a cell that built, counted and printed its expected value.
STUBS = {
    "build2.sh": 'touch "$OUT/$1.$2.$3.o"\n',
    "expect.py": "import sys\nprint('' if sys.argv[1].startswith('--') else 'ok')\n",
    "matrix_kb.py": "def kernel_bytes(path):\n    return 4\n",
    "count.py": "print('0 1')\n",
}


class Libcalls(unittest.TestCase):
    def test_a_missing_llvm_objdump_leaves_the_cell_unmeasured(self):
        # Its error went to /dev/null: every cell read as calling no runtime function, so a count that left out a
        # side's memset was compared with one that held the inlined fill.
        with tempfile.TemporaryDirectory() as work:
            tools, out = os.path.join(work, "tools"), os.path.join(work, "out")
            os.makedirs(tools)
            for name in ("cell.sh", "libcalls.py"):
                shutil.copy(os.path.join(HERE, name), tools)
            for name, text in STUBS.items():
                with open(os.path.join(tools, name), "w") as stub:
                    stub.write(text)
            env = {**os.environ, "OUT": out, "V": os.path.join(work, "nowhere")}
            env.pop("UNICORN_PYTHON", None)
            run = subprocess.run(["bash", os.path.join(tools, "cell.sh"), "p", "2", "cc"], capture_output=True, text=True, env=env)
            row = run.stdout.split("\t")
            self.assertTrue(row[6].startswith("unmeasured:libcalls"), run.stdout + run.stderr)


if __name__ == "__main__":
    unittest.main()
