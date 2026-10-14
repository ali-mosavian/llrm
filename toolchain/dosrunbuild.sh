#!/bin/sh
# Build the headless DOSBox-X the e2e tests run on (the fork's dosrun branch)
# at a pinned commit, and link it into the directory given. Run by build.rs.
set -eu
# Cargo's build-script environment is not the fork's to see.
unset DEBUG TARGET HOST PROFILE OPT_LEVEL

DEST="$1"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/llrm"
COMMIT=88b2f392d7b36ed790739d796f79c3d538159b64

. "$(dirname "$0")/cache.sh"
dosbox() {
    git clone -q --filter=blob:none -b dosrun https://github.com/ali-mosavian/dosbox-x.git "$1"
    git -C "$1" checkout -q "$COMMIT"
    ( cd "$1" && sh build-dosrun.sh )
}
TREE="$CACHE/dosbox-x-$COMMIT"
cached "$TREE" dosbox

mkdir -p "$DEST"
ln -sf "$TREE/src/dosbox-x" "$DEST/dosbox-x"
