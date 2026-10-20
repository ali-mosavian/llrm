"""The checked-in libfunc table is what the generator makes of the pin's TargetLibraryInfo.td."""
import subprocess
import sys
from pathlib import Path

import gen_libfunc


def test_the_table_is_not_stale():
    if not gen_libfunc.SRC.is_file():
        return
    done = subprocess.run([sys.executable, str(Path(gen_libfunc.__file__)), "--check"], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
