"""Link objects against the Open Watcom C library tools/build-ow-clib.sh built, with Open Watcom's wlink.

    wat_link.py m16|m32 OWDIR EXE OBJECT...

OWDIR is the tree the build ran in; WATCOM names an installed Open Watcom (its wlink and its math libraries).
LLRM_CC names the llrm-cc that compiles floatm.c (llrm-cc sets it to itself).
m16: DOS .EXE, medium model; m32: DOS/32A LE.
"""
import os
import subprocess
import sys
import tempfile
from pathlib import Path

if len(sys.argv) < 5:
    sys.exit(__doc__)
model, ow, exe, objects = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]).resolve(), [Path(one).resolve() for one in sys.argv[4:]]
watcom = Path(os.environ.get("WATCOM", Path.home() / "dos/devtools/dev/c/watcom"))
clib = ow / "bld/clib"
if model == "m16":
    lines = ["format dos", f"libpath {clib}/library/msdos.086/mm", f"libpath {ow}/bld/mathlib/library/msdos.287/mm", f"libpath {watcom}/lib286", f"libpath {watcom}/lib286/dos",
             f"file {clib}/startup/library/msdos.086/mm/cstrt086.obj"]
    libraries = [f"{clib}/library/msdos.086/mm/clibm.lib", f"{ow}/bld/mathlib/library/msdos.287/mm/math87m.lib", "noemu87.lib"]
elif model == "m32":
    stub = ow / "bld/redist/dos32a/stub32a.exe"
    lines = ["format os2 le", f"option stub={stub}", f"libpath {clib}/library/msdos.386/ms_r", f"libpath {ow}/bld/mathlib/library/msdos.387/ms_r", f"libpath {watcom}/lib386",
             f"file {clib}/startup/library/msdos.386/ms_r/cstrt386.obj"]
    libraries = [f"{clib}/library/msdos.386/ms_r/clib3r.lib", f"{ow}/bld/mathlib/library/msdos.387/ms_r/math387r.lib", "noemu387.lib"]
else:
    sys.exit(__doc__)
ROOT = Path(__file__).resolve().parent.parent
floatm = Path(tempfile.mkdtemp()) / "floatm.obj"
compiled = subprocess.run([os.environ.get("LLRM_CC", str(ROOT / "target/release/llrm-cc")), "-O1", "-c", str(ROOT / "toolchain/clang/floatm.c"), "-o", str(floatm)], capture_output=True, text=True, env={**os.environ, "LLRM_CC_TARGET": f"x86_{'16' if model == 'm16' else '32'}-wat"})
if compiled.returncode:
    sys.exit(compiled.stderr)
objects.append(floatm)
lines += ["reference _fltused_", f"option quiet", f"name {exe}"] + [f"file {one}" for one in objects] + [f"library {name}" for name in libraries]
with tempfile.TemporaryDirectory() as work:
    response = Path(work) / "link.lnk"
    response.write_text("\n".join(lines) + "\n")
    done = subprocess.run([str(watcom / "binl64/wlink"), f"@{response}"], capture_output=True, text=True, env={**os.environ, "WATCOM": str(watcom)})
sys.stdout.write(done.stdout)
sys.stderr.write(done.stderr)
sys.exit(done.returncode)
