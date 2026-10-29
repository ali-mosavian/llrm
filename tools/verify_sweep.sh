#!/usr/bin/env bash
# A pass that leaves a malformed phi fails here, not as a consumer's runaway
# allocation (#3016): compiles a small corpus with LLRM_VERIFY on (the module
# is verified after every pass, `after <pass>: ...`), at -O2 and -Os.
#
#   tools/verify_sweep.sh [LLRM_CC] [EXTRA.c ...]
#
# LLRM_CC defaults to the resolver's llrm-cc (tools/llrmbin.py). EXTRA sources (d_faces, a QCport
# sample) are compiled as m32 sysv i486 too. At most 40 compiles.
set -u
cd "$(dirname "$0")/.."
llrm_cc=${1:-$(python3 tools/llrmbin.py bin)/llrm-cc}
[ $# -gt 0 ] && shift
kernels=$(printf "%s\n" crates/target/*/vsgcc/kernels | head -1)  # the target that ships the kernels
sources=()
for name in x_switch x_switch_dense x_switch_ranges x_switch_sparse x_hanoi2 x_life x_dct x_crc32 x_gcd x_heapsort x_mergesort x_nbody_f x_matmul_f x_cordic x_fir x_histo2; do
    sources+=("$kernels/$name/$name.c")
done
sources+=("$@")
failed=0 ran=0
for source in "${sources[@]}"; do
    for level in -O2 -Os; do
        [ "$ran" -lt 40 ] || break 2
        ran=$((ran + 1))
        out=$(ulimit -v 6000000; LLRM_VERIFY=1 timeout 120 "$llrm_cc" "$level" --target=x86_32-wat -march=i486 -S -o /dev/null -I "$(dirname "$source")" "$source" 2>&1)
        if [ $? -ne 0 ]; then
            echo "FAIL $level $source: $(printf '%s' "$out" | grep -m1 -E 'after [a-z0-9@._() -]+:|error|panicked|memory allocation' | cut -c1-200)"
            failed=$((failed + 1))
        fi
    done
done
echo "verify sweep: $ran compiles, $failed failed"
[ "$failed" -eq 0 ]
