"""Link objects against Borland C++ 3.1's start-up and medium-model libraries with jwlink.

    bcc_link.py LIBDIR EXE OBJECT...
"""
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

if len(sys.argv) < 4:
    sys.exit(__doc__)
lib, exe, objects = Path(sys.argv[1]), Path(sys.argv[2]).resolve(), [Path(one).resolve() for one in sys.argv[3:]]
lines = ["format dos", "option quiet", f"name {exe}", f"file '{lib / 'C0M.OBJ'}'"]
lines += [f"file '{one}'" for one in objects]
floatm = Path(tempfile.mkdtemp()) / "floatm.obj"
compiled = subprocess.run([os.environ.get("LLRM_CC_BIN") or os.environ.get("LLRM_CC") or str(ROOT / "target/release/llrm-cc"), "-O1", "-c", str(ROOT / "toolchain/clang/floatm.c"), "-o", str(floatm)], capture_output=True, text=True, env={**os.environ, "LLRM_CC_TARGET": "x86_16-bcc"})
if compiled.returncode:
    sys.exit(compiled.stderr)
lines.append(f"file '{floatm}'")
lines += [f"library '{lib / name}'" for name in ("CM.LIB", "MATHM.LIB", "FP87.LIB")]
with tempfile.TemporaryDirectory() as work:
    response = Path(work) / "link.lnk"
    response.write_text("\n".join(lines) + "\n")
    done = subprocess.run(["jwlink", f"@{response}"], capture_output=True, text=True)
sys.stdout.write(done.stdout)
sys.stderr.write(done.stderr)
sys.exit(done.returncode)
