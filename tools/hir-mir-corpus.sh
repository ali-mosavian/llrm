#!/usr/bin/env bash
# Step 4's instrument (docs/architecture/rich-mir.md): every QB suite
# program's and Nib fixture's HIR emitted as MIR, which llrm-mir's verifier
# and opt must both accept, and which must hold no poison its language
# defines; what the emitter refuses is counted by reason.
set -u
bin=${LLVM20:-/usr/lib/llvm-20/bin}
root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$root/target/hir-mir}
cargo build -q --release -p llrm-qb -p llrm-nib -p llrm-hir --manifest-path "$root/Cargo.toml" 2>/dev/null || { echo "hir-mir-corpus: build fails" >&2; exit 2; }
rm -rf "$out" && mkdir -p "$out"
failed=0
emit() {
  "$root/target/release/hir-mir" "$out/$1.json" >"$out/$1.ll" 2>"$out/$1.err" || { echo "FAIL $1: $(grep -m1 -e "^invalid" -e "^poison" "$out/$1.err")"; failed=1; }
  "$bin/opt" -passes=verify -disable-output "$out/$1.ll" 2>"$out/$1.opt" || { echo "FAIL $1: opt rejects it: $(head -1 "$out/$1.opt")"; failed=1; }
}
for source in "$root"/tests/suite/*.bas; do
  name=qb-$(basename "$source" .bas)
  if "$root/target/release/llrm-qb" "$source" --dump-hir "$out/$name.json" --mir >/dev/null 2>"$out/$name.frontend"; then
    emit "$name"
  else
    echo "frontend fails: $name"
  fi
done
for source in $(find "$root/tests/fixtures/nib" -name '*.nib' | sort); do
  name=nib-$(echo "${source#"$root"/tests/fixtures/nib/}" | tr / - | sed 's/\.nib$//')
  if "$root/target/release/llrm-nib" "$source" --dump "$out/$name.d" >/dev/null 2>"$out/$name.frontend"; then
    cp "$out/$name.d/03-hir.json" "$out/$name.json"
    emit "$name"
  else
    echo "frontend fails: $name"
  fi
done
echo "programs: $(ls "$out"/*.frontend | wc -l), emitted: $(ls "$out"/*.ll | wc -l)"
echo "defined: $(cat "$out"/*.ll | grep -c '^define')"
sed -n 's/^refused @[^:]*: //p' "$out"/*.err | sed 's/[0-9][0-9]*/N/g' | sort | uniq -c | sort -rn
exit $failed
