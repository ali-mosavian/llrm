"""Link objects for TARGET's C start-up and OS layer into EXE; the loader it needs beside it is `run_exe.py`'s to copy.

    link_objects.py [--entry NAME] TARGET EXE OBJECT...

--entry names the symbol the start-up calls (`main_` for a program in the register convention); default `_main`.
"""
import os
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dosbatch  # noqa: E402

args = sys.argv[1:]
entry = None
if args[:1] == ["--entry"] and len(args) > 1:
    entry, args = args[1], args[2:]
if len(args) < 3:
    sys.exit(__doc__)
target, exe, first, *rest = args[0], Path(args[1]).resolve(), Path(args[2]).resolve(), *args[3:]
with tempfile.TemporaryDirectory() as work:
    # LLRM_NEAR_CODE: the program's code is near (small and compact models): the start-up and the externals are linked near.
    defines = dosbatch.entry_defines(target, entry, near=bool(os.environ.get("LLRM_NEAR_CODE")))
    dosbatch.link_target(target, first, exe, Path(work), objects_after=tuple(Path(one).resolve() for one in rest), defines=defines)
