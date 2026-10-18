#!/bin/bash
# Stage the pinned toolchain (what tools/llvmpin.py uses, nothing else) for `sudo cp -a STAGE /opt/llrm-pin`:
#   bin/{clang-24,clang,opt,llc}  lib/clang/24 (the resource dir)  gcc/{bin,libexec,lib} (gcc 13.4.0 relocatable from gcc/bin)
# Verified from the staged path by llvmpin's own version checks; prints the manifest size.
#   tools/ci/stage_pin.sh [STAGE=~/scratch/llrm-pin-stage] [LLVM_BUILD=~/scratch/llvm-build]
set -euo pipefail
stage=${1:-$HOME/scratch/llrm-pin-stage}
llvm=${2:-$HOME/scratch/llvm-build}
here=$(cd "$(dirname "$0")/../.." && pwd)
triple=x86_64-linux-gnu
rm -rf "$stage"; mkdir -p "$stage/bin" "$stage/lib/clang" "$stage/gcc/bin" "$stage/gcc/libexec/gcc/$triple" "$stage/gcc/lib/gcc/$triple"
cp -a "$llvm/bin/clang-24" "$llvm/bin/opt" "$llvm/bin/llc" "$stage/bin/"
ln -s clang-24 "$stage/bin/clang"
cp -a "$llvm/lib/clang/24" "$stage/lib/clang/"
cp -a "/usr/bin/$triple-gcc-13" "$stage/gcc/bin/"
ln -s "$triple-gcc-13" "$stage/gcc/bin/gcc"; ln -s "$triple-gcc-13" "$stage/gcc/bin/gcc-13"
cp -a "/usr/libexec/gcc/$triple/13" "$stage/gcc/libexec/gcc/$triple/"
cp -a "/usr/lib/gcc/$triple/13" "$stage/gcc/lib/gcc/$triple/"
# gcc's dev links to the system's sanitizer/runtime libraries point out of the stage: dropped, so a -fsanitize/-fopenmp link fails plainly (no pin test uses them).
find "$stage" -xtype l -delete
python3 "$here/tools/ci/check_stage.py" "$stage"
(cd "$stage" && find . -type f -printf '%s %p\n' | sort -k2 > MANIFEST && sha256sum $(find . -type f ! -name MANIFEST | sort) >> MANIFEST)
# Verified from the staged path: the pinned build's version, gcc 13.4.0, and a compile with each.
export LLRM_PIN_DIR=$stage PATH="$stage/gcc/bin:$PATH"
python3 "$here/tools/llvmpin.py" path clang opt llc >/dev/null 2>&1 || true
for t in clang opt llc; do [ "$(python3 "$here/tools/llvmpin.py" path $t)" = "$stage/bin/$t" ] || { echo "llvmpin does not resolve $t to the stage" >&2; exit 1; }; done
"$stage/bin/clang" --version | head -1 | grep -q d1106deb71cc || { echo "staged clang is not d1106deb71cc" >&2; exit 1; }
[ "$(command -v gcc)" = "$stage/gcc/bin/gcc" ] && gcc --version | head -1 | grep -q 13.4.0 || { echo "staged gcc is not 13.4.0 first on PATH" >&2; exit 1; }
t=$(mktemp -d); echo 'int main(void){return 0;}' > "$t/a.c"
gcc -c "$t/a.c" -o "$t/a.o" && "$stage/bin/clang" -c "$t/a.c" -o "$t/b.o" && rm -rf "$t"
echo "staged $(du -sh "$stage" | cut -f1) in $stage ($(grep -c . "$stage/MANIFEST") lines, MANIFEST); verified from the stage"
echo "sudo cp -a $stage /opt/llrm-pin && sudo chmod -R a+rX /opt/llrm-pin"
