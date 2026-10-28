"""Link objects against the Open Watcom C library tools/build-ow-clib.sh built, with the wlink of the tree it was built in (an installed Open Watcom if that has none).

    wat_link.py m16|m32 OWDIR EXE OBJECT...

OWDIR is the tree the build ran in; WATCOM names an installed Open Watcom, used only when OWDIR has no bwlink or no noemu stubs.
m16: DOS .EXE, medium model; m32: DOS/32A LE.
"""
import os
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def llrm_cc(env, root):
    """The llrm-cc that builds the runtime's floatm: LLRM_CC_BIN or LLRM_CC, else LLRM_BIN, else the newest of the tree's release and debug."""
    if env.get("LLRM_CC_BIN") or env.get("LLRM_CC"):
        return Path(env.get("LLRM_CC_BIN") or env["LLRM_CC"])
    if env.get("LLRM_BIN"):
        return Path(env["LLRM_BIN"]) / "llrm-cc"
    built = [root / "target" / kind / "llrm-cc" for kind in ("release", "debug")]
    built = [one for one in built if one.is_file()]
    return max(built, key=lambda one: one.stat().st_mtime) if built else root / "target/release/llrm-cc"


def toolset(ow, environ, home):
    """(linker, 16-bit library directories, 32-bit library directories): the linker and the no-emulation stubs of the Open Watcom tree the library was
    built in (`./build.sh boot` makes bwlink, bld/fpuemu the stubs), else an installed Open Watcom (WATCOM, default ~/dos/devtools/dev/c/watcom)."""
    own = ow / "build/binbuild/bwlink", [ow / "bld/fpuemu/i86/stubi86"], [ow / "bld/fpuemu/386/stub386"]
    if own[0].exists() and (own[1][0] / "noemu87.lib").exists() and (own[2][0] / "noemu387.lib").exists():
        return own
    watcom = Path(environ.get("WATCOM") or home / "dos/devtools/dev/c/watcom")
    return watcom / "binl64/wlink", [watcom / "lib286", watcom / "lib286/dos"], [watcom / "lib386"]


def main():
    if len(sys.argv) < 5:
        sys.exit(__doc__)
    model, ow, exe, objects = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3]).resolve(), [Path(one).resolve() for one in sys.argv[4:]]
    linker, libs16, libs32 = toolset(ow, os.environ, Path.home())
    clib = ow / "bld/clib"
    if model == "m16":
        lines = ["format dos", f"libpath {clib}/library/msdos.086/mm", f"libpath {ow}/bld/mathlib/library/msdos.287/mm", *(f"libpath {one}" for one in libs16),
                 f"file {clib}/startup/library/msdos.086/mm/cstrt086.obj"]
        libraries = [f"{clib}/library/msdos.086/mm/clibm.lib", f"{ow}/bld/mathlib/library/msdos.287/mm/math87m.lib", "noemu87.lib"]
    elif model == "m32":
        stub = ow / "bld/redist/dos32a/stub32a.exe"
        lines = ["format os2 le", f"option stub={stub}", f"libpath {clib}/library/msdos.386/ms_r", f"libpath {ow}/bld/mathlib/library/msdos.387/ms_r", *(f"libpath {one}" for one in libs32),
                 f"file {clib}/startup/library/msdos.386/ms_r/cstrt386.obj"]
        libraries = [f"{clib}/library/msdos.386/ms_r/clib3r.lib", f"{ow}/bld/mathlib/library/msdos.387/ms_r/math387r.lib", "noemu387.lib"]
    else:
        sys.exit(__doc__)
    floatm = Path(tempfile.mkdtemp()) / "floatm.obj"
    compiled = subprocess.run([str(llrm_cc(os.environ, ROOT)), "-O1", "-c", str(ROOT / "toolchain/clang/floatm.c"), "-o", str(floatm)], capture_output=True, text=True, env={**os.environ, "LLRM_CC_TARGET": f"x86_{'16' if model == 'm16' else '32'}-wat"})
    if compiled.returncode:
        sys.exit(compiled.stderr)
    objects.append(floatm)
    lines += ["reference _fltused_", f"option quiet", f"name {exe}"] + [f"file {one}" for one in objects] + [f"library {name}" for name in libraries]
    with tempfile.TemporaryDirectory() as work:
        response = Path(work) / "link.lnk"
        response.write_text("\n".join(lines) + "\n")
        done = subprocess.run([str(linker), f"@{response}"], capture_output=True, text=True, env=os.environ)
    sys.stdout.write(done.stdout)
    sys.stderr.write(done.stderr)
    sys.exit(done.returncode)


if __name__ == "__main__":
    main()
