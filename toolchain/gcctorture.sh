#!/bin/sh
# GCC's gcc.c-torture/execute (tools/torture) at a pinned commit, cached per commit under ~/.cache/llrm.
# Prints the directory to use as TORTURE_CORPUS.
set -eu
# stdout is the directory, alone: the setup action captures it (`TORTURE_CORPUS=$(sh toolchain/gcctorture.sh)`). Every other output goes to stderr.
exec 3>&1 1>&2
COMMIT=416290b10bb9cb45361630b61af09200962bd5a5
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/llrm"

. "$(dirname "$0")/cache.sh"
corpus() {
    git init -q "$1/gcc"
    git -C "$1/gcc" fetch -q --depth 1 --filter=blob:none https://github.com/gcc-mirror/gcc.git "$COMMIT"
    git -C "$1/gcc" sparse-checkout set gcc/testsuite/gcc.c-torture/execute
    git -C "$1/gcc" checkout -q FETCH_HEAD
}
cached "$CACHE/gcc-torture-$COMMIT" corpus
echo "$CACHE/gcc-torture-$COMMIT/gcc/gcc/testsuite/gcc.c-torture/execute" >&3
