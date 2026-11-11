"""The object compare reads code bytes: an object that grows by an EXTDEF while its code shrinks reads smaller."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import objcmp  # noqa: E402

DATA = Path(__file__).resolve().parent / "testdata" / "objcmp"
BASE, HEAD = DATA / "os_execute.base.obj", DATA / "os_execute.head.obj"


def test_the_files_grew_while_the_code_shrank():
    """tests/run/c/os_execute.c, m32 -O1, #8732: `rep movs` became `call memcpy`. The OMF files grow 10 B (the memcpy EXTDEF +9, fixups
    +4, code -3); a compare of file sizes read that as growth."""
    assert HEAD.stat().st_size - BASE.stat().st_size == 10, "premise: the file grew"
    assert objcmp.code_delta(BASE, HEAD) == -3


def test_identical_files_are_same_and_a_changed_pair_is_not():
    assert objcmp.compare(BASE, BASE) == (True, 0)
    assert objcmp.compare(BASE, HEAD) == (False, -3)


def test_without_the_decoder_the_import_fails_loudly():
    """The corpus A/B ran outside `uv`, no `iced_x86`: every code delta came back "unread" and six changed cells showed file deltas only."""
    import subprocess
    import sys

    code = f"import sys; sys.modules['iced_x86'] = None; sys.path.insert(0, {str(Path(__file__).resolve().parent)!r}); import objcmp"
    run = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True)
    assert run.returncode != 0 and "INSTRUMENT: iced_x86 missing" in run.stderr, run.stderr
