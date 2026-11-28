"""wrapper.py's llrm command, checked without QCport (test_dfaces.py needs its sources)."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import wrapper  # noqa: E402


def test_llrm_compiles_d_faces_for_a_named_flat_row():
    """`-m32` named no llrm-cc row: the compile built i386 IR against the x86-64 table and the smoke's d_faces step "did not run" on every merge."""
    command = wrapper.llrm_command(Path("llrm-cc"), "O2", Path("q"), Path("d.c"), Path("d.obj"))
    assert "-m32" not in command and "-c" in command, "an object, not a link"
    assert "--target=x86_32-wat" in command
