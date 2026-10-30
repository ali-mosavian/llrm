#!/bin/sh
# Open Watcom's DOS C library built from source with make -j: tools/build-ow-clib.sh [DIR] (default ~/scratch/ow-src)
# DIR is a copy of the Open Watcom tree toolchain/owshim/ow-commit pins (~/.cache/llrm/open-watcom-v2-<commit>, bootstrapped by
# toolchain/wdump.sh). The build runs on that tree's own bw* tools; an installed Open Watcom (WATCOM) is not needed, and its binl64 is
# put on the path only when WATCOM is set. Makes DIR/bld/clib/library/msdos.086/mm/clibm.lib
# (16-bit, medium, register calls) and msdos.386/ms_s/clib3s.lib (32-bit, small, stack calls) and msdos.386/ms_r/clib3r.lib (register calls, llrm's `watcom` ABI).
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
ow=${1:-$HOME/scratch/ow-src}
cache=${XDG_CACHE_HOME:-$HOME/.cache}/llrm
[ -d "$ow/bld" ] || { mkdir -p "$ow"; cp -a "$cache/open-watcom-v2-$(cat "$root/toolchain/owshim/ow-commit")/." "$ow"; }
export OWROOT=$ow OWOBJDIR=binbuild OWTOOLS=WATCOM OWTOOLSVER=1300
export OWBLDVER=20 OWBLDVERSTR=2.0 OWBLDVERTOOL=1300 DEVDIR=$ow/bld
export PATH=$ow/build/binbuild:$ow/build:${WATCOM:+$WATCOM/binl64:}$PATH
(cd "$ow/bld/hdr/dos" && wmake -h >/dev/null)
# wmake's dry run lists only what is out of date: generate from clean directories.
find "$ow/bld/clib" "$ow/bld/mathlib" \( -name '*.obj' -o -name '*.lib' -o -name '*.lbc' \) -path '*library/msdos*' -delete
python3 "$root/tools/ow-clib-make.py" "$ow" "$ow/clib.mk" msdos.086/mm msdos.386/ms_r msdos.386/ms_s \
  mathlib:msdos.387/ms_r mathlib:msdos.287/mm
make -f "$ow/clib.mk" -j"${JOBS:-8}" -k
# The no-emulation stubs wat_link.py links last (noemu87.lib, noemu387.lib), from the same tree.
for stub in i86/stubi86 386/stub386; do (cd "$ow/bld/fpuemu/$stub" && wmake -h >/dev/null); done
