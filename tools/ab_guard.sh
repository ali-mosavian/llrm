#!/bin/bash
# A/B guard: ab_guard.sh <repo> <base> <head> <base-bin> <head-bin> [counts-file...]
# Refuses (exit 2, one line) unless base is the newest upstream commit head contains
# (upstream: $AB_UPSTREAM, default origin/release/next), the binaries differ, every counts file is non-empty,
# and $AB_BASE_ARGV / $AB_HEAD_ARGV (the command lines the two sides run) each carry an explicit -O level:
# llrm-wasm-llc once defaulted to -O0 against the pin's -O2 and a phantom gap was chased. Both lines are printed.
# Any LLRM_* variable outside ENV_OK (tool locations) is refused unless $AB_ALLOW_ENV names it: LLRM_WASM_BACKEND_ONLY
# skipped the mid-end on one side and a second cause was chased for 40 minutes. The LLRM_* environment is printed.
repo=$1 base=$2 head=$3 bbin=$4 hbin=$5; shift 5
no() { echo "ab_guard: $*" >&2; exit 2; }
up=${AB_UPSTREAM:-origin/release/next}
b=$(git -C "$repo" rev-parse --verify -q "$base^{commit}") || no "base $base is not a commit"
h=$(git -C "$repo" rev-parse --verify -q "$head^{commit}") || no "head $head is not a commit"
want=$(git -C "$repo" merge-base "$h" "$up") || no "no merge-base of head and $up"
[ "$b" = "$want" ] || no "stale base: base ${b:0:10} is not the newest $up commit head contains (${want:0:10})"
[ "$(md5sum < "$bbin")" != "$(md5sum < "$hbin")" ] || no "base and head binaries are identical"
for c in "$@"; do [ -s "$c" ] || no "counts file $c is empty or missing"; done
for side in BASE HEAD; do
  var=AB_${side}_ARGV
  [ -n "${!var:-}" ] || no "no argv given for the ${side,,} side: $var is not set; say the command line it runs"
  [[ " ${!var} " =~ \ -O(0|1|2|3|s|z|g|max)\  ]] || no "no explicit -O in the ${side,,} argv: ${!var}"
done
echo "ab_guard: base argv: $AB_BASE_ARGV"
echo "ab_guard: head argv: $AB_HEAD_ARGV"
ENV_OK=" LLRM_OW LLRM_PIN_CLANG LLRM_CLANG_WASM LLRM_ARM64_SYSROOT LLRM_CC_TARGET "
while IFS='=' read -r name value; do
  [[ "$name" == LLRM_* ]] || continue
  [[ "$ENV_OK" == *" $name "* || " ${AB_ALLOW_ENV:-} " == *" $name "* ]] || no "$name=$value changes the pipeline; unset it, or name it in AB_ALLOW_ENV if both sides are meant to run it"
  echo "ab_guard: env: $name=$value"
done < <(env | sort)
