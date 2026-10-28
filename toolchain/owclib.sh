#!/bin/sh
# Open Watcom's DOS C library (LLRM_OW), built from the source tree owshim/ow-commit pins and cached as ~/.cache/llrm/ow-clib-<commit>; its path
# is printed. No installed Open Watcom: the tree's own bw* tools build it (tools/build-ow-clib.sh), the way toolchain/wdump.sh builds wdump.
#   toolchain/owclib.sh           build (cached per commit) and print the directory
#   toolchain/owclib.sh --print   print the directory it would be, building nothing
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
OW_COMMIT=$(cat "$HERE/owshim/ow-commit")
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/llrm"
DIR="$CACHE/ow-clib-$OW_COMMIT"
if [ "${1:-}" = --print ]; then
    echo "$DIR"
    exit 0
fi
# stdout is the path, alone; every build log goes to stderr.
exec 3>&1 1>&2
. "$HERE/cache.sh"

# As wdump.sh and owshim/build.sh make it: the same commit, bootstrapped the same way, so whichever runs first leaves the tree the others use.
ow_tree() {
    git clone -q --filter=blob:none https://github.com/open-watcom/open-watcom-v2.git "$1"
    git -C "$1" checkout -q "$OW_COMMIT"
    ( set +u; cd "$1" && . ./setvars.sh && native_tools && ./build.sh boot )
}
cached "$CACHE/open-watcom-v2-$OW_COMMIT" ow_tree
# build DIR: DIR is empty; build-ow-clib.sh copies the bootstrapped tree into it and builds the libraries and the noemu stubs there.
build() {
    sh "$HERE/../tools/build-ow-clib.sh" "$1"
}
cached "$DIR" build
[ -f "$DIR/bld/clib/library/msdos.386/ms_r/clib3r.lib" ] || { echo "owclib.sh: $DIR has no clib3r.lib" >&2; exit 1; }
echo "$DIR" >&3
