#!/usr/bin/env bash
# tools/wasm_corpus.sh LEVEL FILE.c...: per program, the bytes (Code + Data sections; the last two columns Code alone) and the instructions executed (wasm-interp --trace) of
# llrm-cc -target wasm32 and of the pin's clang at -LEVEL, and whether `report` saw the same values.
#   LLRM_CLANG_WASM  the pin clang built with the WebAssembly target (default: $HOME/scratch/llvm-build-clang-wasm/bin/clang)
#   WASM_CPU         -mcpu for both sides (mvp: the secondary column; default: the pin's generic)
# The stub is the same bytes on both sides: `run` calls main and answers a hash of what main reported.
set -u
ulimit -v 16777216 # one runaway compile must not take the box
root=$(cd "$(dirname "$0")/.." && pwd)
clang=${LLRM_CLANG_WASM:-$HOME/scratch/llvm-build-clang-wasm/bin/clang}
llrm_cc=${LLRM_CC:-$(python3 "$root/tools/llrmbin.py" bin)/llrm-cc}
out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
level=$1; shift
# A broken tool is a failure of the run, not a row: exit 2 without it; exit 1 when no program was measured.
[ -x "$clang" ] && [ -x "$llrm_cc" ] || { echo "wasm_corpus: missing compiler ($clang, $llrm_cc)" >&2; exit 2; }
[ $# -gt 0 ] || { echo "wasm_corpus: no program given" >&2; exit 2; }
cpu=${WASM_CPU:+-mcpu=$WASM_CPU}
cat > "$out/stub.c" <<'EOF'
int __main_void(void) __attribute__((weak));
int __main_argc_argv(int, char **) __attribute__((weak));
static unsigned long state;
void report(long value) { state = state * 31u + (unsigned long)value; }
/* The input of the benches that read one (grep): 64 KB of words, the same bytes on both sides. */
int input_read(char *buffer, int count)
{
    static const char *const words[] = {"Oliver ", "Scrooge ", "ghost ", "Marley ", "and ", "the ", "of ", "Tiny Tim\n", "a "};
    static unsigned long seed = 1, sent;
    enum { input_bytes = 64 << 10 };
    int n = 0;
    while (n < count && sent < input_bytes) {
        const char *w;
        seed = seed * 1103515245u + 12345u;
        for (w = words[(seed >> 16) % 9]; *w && n < count; ++w, ++sent) buffer[n++] = *w;
    }
    return n;
}
int run(void)
{
    if (__main_void) __main_void();
    else __main_argc_argv(0, 0);
    return (int)state;
}
EOF
"$clang" --target=wasm32 -O2 -c "$out/stub.c" -o "$out/stub.o" || { echo "wasm_corpus: the pin clang cannot compile the stub" >&2; exit 2; }

# Section sizes of a relocatable object, summed over its sections of that name.
section() { wasm-objdump -h "$2" | sed -n "s/^ *$1 start=.*(size=0x\\([0-9a-f]*\\)).*/\\1/p" | while read -r h; do echo $((16#$h)); done; }
code() { section Code "$1"; }
# The bytes a cell is judged by: the code and the initialised data it loads (a lookup table, a string).
image() { echo $(($(code "$1") + $(section Data "$1" | tr -d "\n" | grep . || echo 0))); }

# "result instructions" of one object linked with the stub.
count() {
  wasm-ld --no-entry --export=run --allow-undefined "$1" "$out/stub.o" -o "$1.wasm" 2>/dev/null || { echo "none 0"; return; }
  timeout 120 wasm-interp "$1.wasm" -r run --trace > "$1.trace" 2>&1
  echo "$(sed -n 's/^run() => i32:\(-*[0-9]*\)$/\1/p' "$1.trace" | head -1 | grep . || echo none) $(grep -c '^#' "$1.trace")"
}

printf '%-22s %9s %9s %12s %12s %9s %9s  %s\n' program llrm_B clang_B llrm_instr clang_instr llrm_code clang_code result
measured=0
for f in "$@"; do
  n=$(basename "$f" .c)
  LLRM_CLANG_WASM=$clang LLRM_CC_TARGET=wasm32 timeout 300 "$llrm_cc" "-$level" $cpu -c "$f" -I"$root/bench" -I"$root/runtime/shared/posix" -o "$out/$n.l.o" 2> "$out/$n.err" ||
    { printf '%-22s llrm-cc FAIL: %s\n' "$n" "$(head -c 90 "$out/$n.err" | tr '\n' ' ')"; continue; }
  timeout 300 "$clang" --target=wasm32 -ffreestanding "-$level" $cpu -c "$f" -I"$root/bench" -I"$root/runtime/shared/posix" -o "$out/$n.c.o" 2> /dev/null ||
    { printf '%-22s clang FAIL\n' "$n"; continue; }
  read -r lv li <<< "$(count "$out/$n.l.o")"
  read -r cv ci <<< "$(count "$out/$n.c.o")"
  # A side that did not run (unresolved import, trap) has no result: the row is unjudged, never "same".
  if [ "${lv:-none}" = none ] || [ "${cv:-none}" = none ]; then ok="n/a(no result: llrm ${lv:-none}, clang ${cv:-none})"
  elif [ "$lv" = "$cv" ]; then ok=same
  else ok="DIFFERENT($lv/$cv)"; fi
  printf '%-22s %9s %9s %12s %12s %9s %9s  %s\n' "$n" "$(image "$out/$n.l.o")" "$(image "$out/$n.c.o")" "${li:-0}" "${ci:-0}" \
    "$(code "$out/$n.l.o")" "$(code "$out/$n.c.o")" "$ok"
  measured=$((measured + 1))
done
[ "$measured" -gt 0 ] || { echo "wasm_corpus: no program was measured" >&2; exit 1; }
