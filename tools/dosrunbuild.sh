#!/bin/sh
# Build the headless DOSBox-X the e2e tests run on (the fork's dosrun branch)
# at a pinned commit, and link it into the directory given. Run by build.rs.
set -eu
# Cargo's build-script environment is not the fork's to see.
unset DEBUG TARGET HOST PROFILE OPT_LEVEL

DEST="$1"
TREE="${XDG_CACHE_HOME:-$HOME/.cache}/llrm/dosbox-x"
COMMIT=36b738a39b5a08a9539a3235f6a7c322208b76df

[ -d "$TREE/.git" ] || git clone -q --filter=blob:none -b dosrun https://github.com/ali-mosavian/dosbox-x.git "$TREE"
if [ "$(git -C "$TREE" rev-parse HEAD)" != "$COMMIT" ]; then
    git -C "$TREE" fetch -q origin dosrun
    git -C "$TREE" checkout -q "$COMMIT"
    rm -f "$TREE/Makefile"
fi
cd "$TREE"
if [ -f Makefile ]; then make -s -j"$(nproc)"; else sh build-dosrun.sh; fi

mkdir -p "$DEST"
ln -sf "$TREE/src/dosbox-x" "$DEST/dosbox-x"
