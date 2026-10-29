#!/bin/sh
# Build Open Watcom's wdump, the dump tool tests/dwarf.rs reads the 16-bit DWARF images with, and print its path.
# Built as a tool from the tree owshim/ow-commit pins (the same tree the Open Watcom library is built from, cloned and bootstrapped once,
# cached per commit under ~/.cache/llrm): its bootstrap makefile and the host compiler, a second on a bootstrapped tree.
# OWROOT is an Open Watcom tree: the user's own when set. Nothing of Open Watcom's is copied into this repository.
set -eu
# stdout is the path, alone: the callers capture it (`wdump=$(sh toolchain/wdump.sh)` in the setup action). Every build log goes to stderr.
exec 3>&1 1>&2

HERE="$(cd "$(dirname "$0")" && pwd)"
. "$HERE/cache.sh"
OW_COMMIT=$(cat "$HERE/owshim/ow-commit")
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/llrm"

# As owshim/build.sh makes it: the same commit, bootstrapped the same way, so whichever runs first leaves the tree the other uses.
ow_tree() {
    git clone -q --filter=blob:none https://github.com/open-watcom/open-watcom-v2.git "$1"
    git -C "$1" checkout -q "$OW_COMMIT"
    ( set +u; cd "$1" && . ./setvars.sh && native_tools && ./build.sh boot )
}
if [ -z "${OWROOT:-}" ]; then
    OWROOT="$CACHE/open-watcom-v2-$OW_COMMIT"
    cached "$OWROOT" ow_tree
elif [ ! -x "$OWROOT/build/binbuild/wmake" ]; then
    ( set +u; cd "$OWROOT" && . ./setvars.sh && native_tools && ./build.sh boot )
fi

# wdump DIR: bld/exedump built with the bootstrapped wmake and the host compiler, as the front end's objects are, and copied to DIR.
wdump() {
    # `cached` runs this in an `if`, where `set -e` is off: each step says so itself, and a wdump.exe left by an earlier build is not one.
    rm -f "$OWROOT/bld/exedump/binbuild/wdump.exe"
    ( set +u; cd "$OWROOT" && . ./setvars.sh >/dev/null && native_tools && cd bld/exedump && mkdir -p binbuild && cd binbuild \
        && "$OWROOT/build/binbuild/wmake" -h -f ../binmake bootstrap=1 >&2 ) || return 1
    # Open Watcom's makefile names a host build wdump.exe on every host; it is the host's own format (ELF, Mach-O), which this checks: a
    # DOS or Windows image would not run here. Installed without the extension.
    built="$OWROOT/bld/exedump/binbuild/wdump.exe"
    magic=$(od -An -tx1 -N4 "$built" | tr -d ' \n')
    case "$(uname -s):$magic" in
        Linux:7f454c46 | Darwin:cffaedfe | Darwin:cefaedfe | Darwin:feedfacf | Darwin:feedface | Darwin:cafebabe) ;;
        *) echo "wdump.sh: $built is not a $(uname -s) executable (starts $magic)" >&2; return 1 ;;
    esac
    cp "$built" "$1/wdump"
}
cached "$CACHE/wdump-$OW_COMMIT" wdump
echo "$CACHE/wdump-$OW_COMMIT/wdump" >&3
