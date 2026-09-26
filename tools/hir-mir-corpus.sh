#!/usr/bin/env bash
# Step 4's instrument (docs/architecture/rich-mir.md): every QB suite
# program's HIR emitted as MIR, which llrm-mir's verifier and opt must both
# accept; what the emitter refuses is counted by reason.
set -u
bin=${LLVM20:-/usr/lib/llvm-20/bin}
root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$root/target/hir-mir}
cargo build -q --release -p llrm-qb -p llrm-hir --manifest-path "$root/Cargo.toml" 2>/dev/null || { echo "hir-mir-corpus: build fails" >&2; exit 2; }
rm -rf "$out" && mkdir -p "$out"
failed=0
for source in "$root"/tests/suite/*.bas; do
  name=$(basename "$source" .bas)
  if ! "$root/target/release/llrm-qb" "$source" --dump-hir "$out/$name.json" --mir >/dev/null 2>"$out/$name.qb"; then
    echo "frontend fails: $name"
    continue
  fi
  "$root/target/release/hir-mir" "$out/$name.json" >"$out/$name.ll" 2>"$out/$name.err" || { echo "FAIL $name: llrm-mir rejects it"; failed=1; }
  "$bin/opt" -passes=verify -disable-output "$out/$name.ll" 2>"$out/$name.opt" || { echo "FAIL $name: opt rejects it: $(head -1 "$out/$name.opt")"; failed=1; }
done
echo "defined: $(cat "$out"/*.ll | grep -c '^define')"
sed -n 's/^refused @[^:]*: //p' "$out"/*.err | sed 's/[0-9][0-9]*/N/g' | sort | uniq -c | sort -rn
exit $failed
