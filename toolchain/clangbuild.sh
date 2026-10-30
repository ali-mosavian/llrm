#!/bin/sh
# The pinned clang, built into ~/.cache/llrm/clang-<sha12> (pin) or clang-m16-<sha12> (m16), and its path printed.
#   toolchain/clangbuild.sh pin|m16          build (cached per sha) and print the install directory
#   toolchain/clangbuild.sh --print pin|m16  print the directory it would be, building nothing
# The shas are toolchain/clang/pin's lines 1 and 2 (TOOLCHAIN_PIN overrides the file): a pin bump names a new tree, so nothing is stale.
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
PIN="${TOOLCHAIN_PIN:-$HERE/clang/pin}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/llrm"

only=0
if [ "${1:-}" = --print ]; then
    only=1
    shift
fi
case "${1:-}" in
pin) line=1 name=clang repo=https://github.com/llvm/llvm-project.git ;;
m16) line=2 name=clang-m16 repo=https://github.com/ali-mosavian/llvm-project.git ;;
*) echo "usage: clangbuild.sh [--print] pin|m16" >&2; exit 2 ;;
esac
sha=$(sed -n "${line}p" "$PIN" | cut -d' ' -f1)
[ "${#sha}" = 40 ] || { echo "clangbuild.sh: line $line of $PIN is not a commit sha" >&2; exit 2; }
DIR="$CACHE/$name-$(echo "$sha" | cut -c1-12)"
if [ "$only" = 1 ]; then
    echo "$DIR"
    exit 0
fi
# stdout is the path, alone; every build log goes to stderr.
exec 3>&1 1>&2
. "$HERE/cache.sh"

# build DIR: clang alone (no static analyzer, no ARCMT: 10% of the work), X86 only, installed with its resource headers.
build() {
    src=$(mktemp -d)
    git init -q "$src"
    git -C "$src" fetch -q --depth 1 "$repo" "$sha"
    git -C "$src" checkout -q FETCH_HEAD
    extra=""
    command -v ld.lld >/dev/null 2>&1 && extra="$extra -DLLVM_USE_LINKER=lld"
    command -v ccache >/dev/null 2>&1 && extra="$extra -DCMAKE_C_COMPILER_LAUNCHER=ccache -DCMAKE_CXX_COMPILER_LAUNCHER=ccache"
    # shellcheck disable=SC2086
    cmake -S "$src/llvm" -B "$src/build" -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$1" \
        -DLLVM_ENABLE_PROJECTS=clang -DLLVM_TARGETS_TO_BUILD=X86 -DLLVM_ENABLE_ASSERTIONS=OFF \
        -DCLANG_ENABLE_ARCMT=OFF -DCLANG_ENABLE_STATIC_ANALYZER=OFF \
        -DLLVM_INCLUDE_TESTS=OFF -DLLVM_INCLUDE_BENCHMARKS=OFF -DLLVM_INCLUDE_EXAMPLES=OFF -DCLANG_INCLUDE_TESTS=OFF $extra || return 1
    ninja -C "$src/build" install-clang install-clang-resource-headers || return 1
    rm -rf "$src"
}
cached "$DIR" build
[ -x "$DIR/bin/clang" ] || { echo "clangbuild.sh: $DIR/bin/clang was not built" >&2; exit 1; }
echo "$DIR" >&3
