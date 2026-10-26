"""Run a linked EXE under the headless DOSBox-X; print its output and exit with its status.

    run_exe.py TARGET EXE
"""
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dosbatch  # noqa: E402

if len(sys.argv) != 3:
    sys.exit(__doc__)
target, exe = sys.argv[1], Path(sys.argv[2]).resolve()
loaders = tuple(Path(one) for one in dosbatch.link_loaders(target))
with tempfile.TemporaryDirectory() as work:
    job = dosbatch.Job("PROG", "exe", exe, files=loaders)
    result = dosbatch.run([job], Path(work))["PROG"]
sys.stdout.write(result.text)
if result.status != "ok":
    sys.exit(f"run_exe: {result.status}: {result.detail}")
sys.exit(result.exit_code or 0)
