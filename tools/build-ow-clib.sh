#!/bin/sh
# Open Watcom's DOS C library built from source with make -j: tools/build-ow-clib.sh [DIR] (default ~/scratch/ow-src)
# DIR is a copy of the Open Watcom tree (made from ~/.cache/llrm/open-watcom-v2-*); WATCOM is an installed Open Watcom
# (default ~/dos/devtools/dev/c/watcom), whose binl64 tools run the build. Makes DIR/bld/clib/library/msdos.086/mm/clibm.lib
# (16-bit, medium, register calls) and msdos.386/ms_s/clib3s.lib (32-bit, small, stack calls).
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
ow=${1:-$HOME/scratch/ow-src}
[ -d "$ow/bld" ] || { mkdir -p "$ow"; cp -a "$(ls -d "$HOME"/.cache/llrm/open-watcom-v2-* | head -1)/." "$ow"; }
export OWROOT=$ow WATCOM=${WATCOM:-$HOME/dos/devtools/dev/c/watcom} OWOBJDIR=binbuild OWTOOLS=WATCOM OWTOOLSVER=1300
export OWBLDVER=20 OWBLDVERSTR=2.0 OWBLDVERTOOL=1300 DEVDIR=$ow/bld
export PATH=$ow/build/binbuild:$ow/build:$WATCOM/binl64:$PATH
(cd "$ow/bld/hdr/dos" && wmake -h >/dev/null)
# wmake's dry run lists only what is out of date: generate from clean directories.
find "$ow/bld/clib" \( -name '*.obj' -o -name '*.lib' -o -name '*.lbc' \) -path '*library/msdos*' -delete
python3 "$root/tools/ow-clib-make.py" "$ow" "$ow/clib.mk" msdos.086/mm msdos.386/ms_s
make -f "$ow/clib.mk" -j"${JOBS:-8}" -k
