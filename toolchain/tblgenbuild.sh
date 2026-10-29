#!/bin/sh
# What tools/dagisel/regen.sh needs of the pin, cached in ~/.cache/llrm/tblgen-<sha12>: the pin's llvm/ source and its llvm-tblgen
# with the generated X86 and IR headers. Not clang, so a cold build is minutes, not an hour.
#   toolchain/tblgenbuild.sh          build (cached per sha) and print the directory: LLVM_SRC=DIR/src LLVM_BUILD=DIR/build
#   toolchain/tblgenbuild.sh --print  print the directory, building nothing
# The sha is toolchain/clang/pin's line 1 (TOOLCHAIN_PIN overrides the file). Assertions do not change the tables (checked), so the build is Release.
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
PIN="${TOOLCHAIN_PIN:-$HERE/clang/pin}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/llrm"
repo=https://github.com/llvm/llvm-project.git

sha=$(sed -n 1p "$PIN" | cut -d' ' -f1)
[ "${#sha}" = 40 ] || { echo "tblgenbuild.sh: line 1 of $PIN is not a commit sha" >&2; exit 2; }
DIR="$CACHE/tblgen-$(echo "$sha" | cut -c1-12)"
if [ "${1:-}" = --print ]; then
    echo "$DIR"
    exit 0
fi
exec 3>&1 1>&2
. "$HERE/cache.sh"

# build DIR: DIR/src is a git checkout of the pin (llvm/, cmake/, third-party/ and the libc/ pieces its CMake names; regen.sh checks its HEAD), DIR/build holds bin/llvm-tblgen and the headers.
build() {
    git init -q "$1/src"
    git -C "$1/src" sparse-checkout set --no-cone /llvm /cmake /third-party /libc/shared /libc/src/__support /libc/hdr /libc/include '!/llvm/test' '!/llvm/unittests' '!/llvm/benchmarks'
    git -C "$1/src" fetch -q --depth 1 --filter=blob:none "$repo" "$sha"
    git -C "$1/src" checkout -q FETCH_HEAD
    extra=""
    command -v ld.lld >/dev/null 2>&1 && extra="$extra -DLLVM_USE_LINKER=lld"
    # shellcheck disable=SC2086
    cmake -S "$1/src/llvm" -B "$1/build" -G Ninja -DCMAKE_BUILD_TYPE=Release -DLLVM_TARGETS_TO_BUILD=X86 \
        -DLLVM_INCLUDE_TESTS=OFF -DLLVM_INCLUDE_BENCHMARKS=OFF -DLLVM_INCLUDE_EXAMPLES=OFF -DLLVM_INCLUDE_DOCS=OFF $extra || return 1
    ninja -C "$1/build" llvm-tblgen intrinsics_gen X86CommonTableGen || return 1
    # Keep what regen.sh reads: bin/llvm-tblgen and the generated headers (the objects are most of the size).
    find "$1/build" -mindepth 1 -maxdepth 1 ! -name bin ! -name include ! -name lib -exec rm -rf {} +
    find "$1/build" \( -name CMakeFiles -o -name '*.o' -o -name '*.a' -o -name '*.d' \) -prune -exec rm -rf {} +
    find "$1/build/bin" -type f ! -name llvm-tblgen -delete
}
cached "$DIR" build
[ -x "$DIR/build/bin/llvm-tblgen" ] || { echo "tblgenbuild.sh: $DIR/build/bin/llvm-tblgen was not built" >&2; exit 1; }
echo "$DIR" >&3
