#!/bin/sh
# Compile every example C file with llrm-cc for each target: tools/cc-sweep.sh [target...]  (x86_16-wat needs LLRM_CLANG_M16 and LLRM_OW: DOS sources see Watcom's libc headers, not the host's)
# Red when a target compiled nothing, wrote no result, or failed more files than tools/cc-sweep.baseline allows (CC_SWEEP_BASELINE overrides the path).
# The baseline is a ratchet: counts may only go down, by hand; the gate rewrites nothing. The failing names are printed so a swap of one FAIL for another shows.
root=$(cd "$(dirname "$0")/.." && pwd)
targets=${*:-x86_16-wat x86_32-dos x86_64-linux arm64-linux}
bin=$(python3 "$root/tools/llrmbin.py" bin)
out=${CC_SWEEP_OUT:-/tmp/cc-sweep}; mkdir -p "$out"
export LLRM_LLC=$bin/llrm-llc
# llrm-cc runs the backend its target row names (llc_program): one missing from the dist fails every file, which reads as "ok 0 fail N".
for t in $targets; do
  llc=$(python3 -c 'import sys, tomllib; print(tomllib.load(open(sys.argv[1], "rb"))[sys.argv[2]].get("llc_program", "llrm-llc"))' "$root/crates/driver/llrm-cc/targets.toml" "$t") || exit 2
  [ -x "$bin/$llc" ] || { echo "cc-sweep: $t needs $llc, which is not in $bin (build it: cargo build --release --bin $llc)" >&2; exit 2; }
done
bad=0; baseline=${CC_SWEEP_BASELINE:-$root/tools/cc-sweep.baseline}
for t in $targets; do
  find "$root/examples" "$root/bench/parity" "$root/tests/llc" "$root/crates/target/llrm-arm64/tests/programs" -name '*.c' |
    xargs -P 6 -I{} sh -c 'f={}; n=$(echo "$f" | md5sum | cut -c1-8)_$(basename "$f" .c); LLRM_CC_TARGET='$t' '"$bin"'/llrm-cc -O1 -c "$f" -o '$out'/'$t'.$n.o 2> '$out'/'$t'.$n.err && echo "ok $f" || echo "FAIL $f"' > "$out/$t.txt"
  if [ ! -s "$out/$t.txt" ]; then echo "$t: no result in $out/$t.txt"; bad=1; continue; fi
  ok=$(grep -c '^ok' "$out/$t.txt"); fail=$(grep -c '^FAIL' "$out/$t.txt")
  base=$(awk -v t="$t" '$1 == t { print $2 }' "$baseline"); base=${base:-0}
  echo "$t: ok $ok fail $fail (baseline $base)"
  grep '^FAIL' "$out/$t.txt" | sed "s#^FAIL $root/#  FAIL #" | sort
  # xargs hides a failing compiler: nothing compiled, or more failures than the baseline, is red.
  [ "$ok" -gt 0 ] || { echo "$t: nothing compiled"; bad=1; }
  [ "$fail" -le "$base" ] || { echo "$t: $fail failures, baseline allows $base"; bad=1; }
  [ "$fail" -ge "$base" ] || echo "$t: below baseline: lower $baseline"
done
exit $bad
