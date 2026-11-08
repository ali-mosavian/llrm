#!/bin/bash
# dag_vs_old.sh TARGET FLAGS [LEVELS] -- Dag vs Old, code bytes per source, same flags both sides.
#   dag_vs_old.sh x86_32-linux "-m32 -march=i486"
#   dag_vs_old.sh x86_64-linux -m64
#   dag_vs_old.sh x86_16-dos -march=i386      (needs LLRM_PIN_CLANG and LLRM_OW)
# Bytes are .text from `size -A` (an ELF object's .eh_frame is not code); an OMF object is its file size.
# Prints the stamp (tip, llrm-llc md5), then per level the cells where Dag is larger, and Dag-only failures.
# Clocks: LLRM_ISEL=old|dag tools/bench/bench.py (tools/bench/dosbatch.py with_isel).
set -u
root=$(git rev-parse --show-toplevel); target=$1; flags=$2; levels=${3:-"-O1 -O2 -Os"}
bin=$root/target/debug; [ -x "$bin/llrm-cc-bin" ] || bin=$root/target/release
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
echo "tip $(git -C "$root" rev-parse HEAD) llrm-llc md5 $(md5sum "$bin/llrm-llc" | cut -c1-12) target $target flags $flags $(date -u +%FT%H:%MZ)"
one() { # SRC LEVEL ISEL
    local n out t
    n=$(basename "$1" .c)_$(echo "$1" | md5sum | cut -c1-6); out=$work/${n}_$2_$3.o
    LLRM_CC_TARGET=$target "$bin/llrm-cc-bin" $flags $2 --isel=$3 -w -I "$root/runtime/shared/posix" -I "$root/bench" \
        -I "$root/runtime/compiler-rt/lib/fp" -c -o "$out" "$1" >/dev/null 2>&1 || { echo "$n FAIL"; return; }
    t=$(size -A "$out" 2>/dev/null | awk '$1==".text"{print $2}')
    echo "$n ${t:-$(stat -c%s "$out")}"; rm -f "$out"
}
export -f one; export root target flags work bin
ls "$root"/tests/run/c/*.c "$root"/bench/*/*.c | sort -u > "$work/list"
for level in $levels; do
    for isel in old dag; do
        xargs -P"${JOBS:-8}" -I{} bash -c "one {} $level $isel" < "$work/list" | sort -u > "$work/$isel"
    done
    echo "== $level Dag larger than Old (name old dag delta)"
    join "$work/old" "$work/dag" | awk '$2!="FAIL"&&$3!="FAIL"&&$3>$2{print $1,$2,$3,$3-$2}' | sed 's/_[0-9a-f]\{6\} / /' | sort -k4 -nr
    echo "== $level Dag fails, Old compiles"
    join "$work/old" "$work/dag" | awk '$3=="FAIL"&&$2!="FAIL"{print $1}' | sed 's/_[0-9a-f]\{6\}$//'
    echo "== $level totals: both $(join "$work/old" "$work/dag" | awk '$2!="FAIL"&&$3!="FAIL"' | wc -l) sources"
done
