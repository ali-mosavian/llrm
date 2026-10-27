"""Link objects for TARGET's C start-up and OS layer into EXE; the loader it needs beside it is `run_exe.py`'s to copy.

    link_objects.py TARGET EXE OBJECT...
"""
import os
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dosbatch  # noqa: E402

if len(sys.argv) < 4:
    sys.exit(__doc__)
target, exe, first, *rest = sys.argv[1], Path(sys.argv[2]).resolve(), Path(sys.argv[3]).resolve(), *sys.argv[4:]
with tempfile.TemporaryDirectory() as work:
    # LLRM_NEAR_CODE: the program's code is near (small and compact models): the start-up and the externals are linked near.
    defines = (*dosbatch.os_defines(target, "c"), "NEAR_CODE=1") if os.environ.get("LLRM_NEAR_CODE") else None
    dosbatch.link_target(target, first, exe, Path(work), objects_after=tuple(Path(one).resolve() for one in rest), defines=defines)
