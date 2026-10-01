#!/bin/sh
# Build jwasm and jwlink at pinned commits of their forks into the directory
# given. Run by build.rs; clones live under ~/.cache/llrm.
set -eu
# Cargo sets DEBUG for build scripts; the makefiles build GccUnixD on any DEBUG.
unset DEBUG

DEST="$1"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/llrm"
JWASM_COMMIT=9fd1afd1a0d6fcebeba975f6586e33356e319e32
JWLINK_COMMIT=4dfdd7896c39b9567f4ea9bfbb7eda601277311e

. "$(dirname "$0")/cache.sh"

# fork NAME COMMIT DIR: the fork at COMMIT in DIR, built there; cached per commit.
fork() {
    git clone -q "https://github.com/ali-mosavian/$1.git" "$3"
    git -C "$3" checkout -q "$2"
    case "$1" in
    JWasm) make -s -C "$3" -f GccUnix.mak ;;
    JWlink)
        for part in dwarf/dw orl sdk/rc/wres .; do
            make -s -C "$3/$part" -f GccUnix.mak
        done
        ;;
    esac
}
cached "$CACHE/JWasm-$JWASM_COMMIT" fork JWasm "$JWASM_COMMIT"
cached "$CACHE/JWlink-$JWLINK_COMMIT" fork JWlink "$JWLINK_COMMIT"

mkdir -p "$DEST"
cp "$CACHE/JWasm-$JWASM_COMMIT/GccUnixR/jwasm" "$CACHE/JWlink-$JWLINK_COMMIT/GccUnixR/jwlink" "$DEST/"
