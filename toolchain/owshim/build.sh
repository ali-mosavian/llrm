#!/bin/sh
# Bootstrap the Open Watcom tree (bwcc, bwcc386, wdump) and print its path.
# OWROOT is the user's own tree when set, else one cached per commit (ow-commit)
# under ~/.cache/llrm, cloned and bootstrapped once, whole or not at all (cache.sh).
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
. "$HERE/../cache.sh"
OW_COMMIT=$(cat "$HERE/ow-commit")
ow_tree() {
    git clone -q --filter=blob:none https://github.com/open-watcom/open-watcom-v2.git "$1"
    git -C "$1" checkout -q "$OW_COMMIT"
    ( set +u; cd "$1" && . ./setvars.sh && native_tools && ./build.sh boot )
}
if [ -z "${OWROOT:-}" ]; then
    OWROOT="${XDG_CACHE_HOME:-$HOME/.cache}/llrm/open-watcom-v2-$OW_COMMIT"
    cached "$OWROOT" ow_tree
elif [ ! -x "$OWROOT/build/binbuild/wmake" ]; then
    ( set +u; cd "$OWROOT" && . ./setvars.sh && native_tools && ./build.sh boot )
fi
echo "$OWROOT"
