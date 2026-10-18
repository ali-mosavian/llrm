#!/bin/sh
# Builds lvi_dump.cpp against the LLVM build of d1106deb71cc, once per LLVM build, and runs it: lvi_dump.sh FILE.ll...
B=${LLVM_BUILD:-$HOME/scratch/llvm-build}
S=${LLVM_SRC:-$HOME/scratch/llvm-src}
BIN=${LVIDUMP_BIN:-$B/lvi_dump}
echo "$("$B/bin/clang++" --version | head -1)" >&2
if [ ! -x "$BIN" ] || [ "$(dirname "$0")/lvi_dump.cpp" -nt "$BIN" ]; then
  "$B/bin/clang++" -std=c++17 -O1 -I"$S/llvm/include" -I"$B/include" "$(dirname "$0")/lvi_dump.cpp" \
    -Wl,--start-group "$B"/lib/libLLVM*.a -Wl,--end-group -lpthread -lz -ldl -o "$BIN" >&2 || exit 1
fi
exec "$BIN" "$@"
