"""The checked-in table of library attributes is what the generator makes of the pin's BuildLibCalls.cpp."""
import subprocess
import sys
from pathlib import Path

import gen_libattrs


def test_the_table_is_not_stale():
    if not gen_libattrs.SRC.is_file():
        return
    done = subprocess.run([sys.executable, str(Path(gen_libattrs.__file__)), "--check"], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr


def test_sin_writes_only_errno_and_cxa_throw_returns_early():
    if not gen_libattrs.SRC.is_file():
        return
    text = gen_libattrs.OUT.read_text()
    assert "set.only_writes_errno_memory();" in text
    assert "return false;" in text
