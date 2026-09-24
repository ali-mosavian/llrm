#!/bin/sh
# Build jwasm and jwlink at pinned commits of their forks into the directory
# given. Run by build.rs; clones live under ~/.cache/llrm.
set -eu
# Cargo sets DEBUG for build scripts; the makefiles build GccUnixD on any DEBUG.
unset DEBUG

DEST="$1"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/llrm"
JWASM_COMMIT=9fd1afd1a0d6fcebeba975f6586e33356e319e32
JWLINK_COMMIT=08eb9b74a1879c065372937e3cdcd7897ffa04b1

# checkout NAME COMMIT: the fork at COMMIT, its build outputs dropped when it moved.
checkout() {
    tree="$CACHE/$1"
    [ -d "$tree/.git" ] || git clone -q "https://github.com/ali-mosavian/$1.git" "$tree"
    if [ "$(git -C "$tree" rev-parse HEAD)" != "$2" ]; then
        git -C "$tree" fetch -q origin
        git -C "$tree" checkout -q "$2"
        find "$tree" -type d -name GccUnixR -prune -exec rm -rf {} +
    fi
}

checkout JWasm "$JWASM_COMMIT"
make -s -C "$CACHE/JWasm" -f GccUnix.mak
checkout JWlink "$JWLINK_COMMIT"
for part in dwarf/dw orl sdk/rc/wres .; do
    make -s -C "$CACHE/JWlink/$part" -f GccUnix.mak
done

mkdir -p "$DEST"
cp "$CACHE/JWasm/GccUnixR/jwasm" "$CACHE/JWlink/GccUnixR/jwlink" "$DEST/"
