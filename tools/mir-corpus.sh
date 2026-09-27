#!/usr/bin/env bash
# The rich-MIR corpus that llrm-analysis's corpus tests read: each program's
# MIR as HIR emits it (emitted/NAME.ll) and after llrm_mir's pipeline
# (optimized/NAME.ll, the last LLRM_MIR_STAGES dump of a compile).
# Covers tests/suite (qb-), the QuickBASIC demos in $QBDEMOS (demo-), Nib's
# fixtures and examples (nib-) and its runtime (nib-runtime). A program is
# left out when a function is refused or the module fails the verifier, and
# a module identical to one already kept is kept once. C has no route:
# llrm-c raises straight to the old MIR.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$root/crates/llrm-analysis/corpus}
demos=${QBDEMOS:-$HOME/work/qbdemos/orig}
cargo build -q --release --bin llrm-qb --bin llrm-nib --bin hir-mir --manifest-path "$root/Cargo.toml" 2>/dev/null || { echo "mir-corpus: build fails" >&2; exit 2; }
bin=$root/target/release
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
rm -rf "$out" && mkdir -p "$out/emitted" "$out/optimized"
declare -A seen
# kept FILE DIR/NAME.ll: FILE copied there unless DIR already holds its text.
kept() {
  local sum; sum=$(dirname "$2"):$(md5sum <"$1" | cut -c1-32)
  [ -z "${seen[$sum]:-}" ] || return 1
  seen[$sum]=1; cp "$1" "$2"
}
# add NAME HIR.json COMPILE...: NAME's emitted MIR, and its optimized MIR
# from COMPILE run with the stage dumps on.
add() {
  local name=$1 hir=$2; shift 2
  if ! "$bin/hir-mir" "$hir" >"$work/$name.ll" 2>"$work/$name.err" || grep -q '^refused' "$work/$name.err"; then
    echo "left out $name: $(head -1 "$work/$name.err")"; return
  fi
  kept "$work/$name.ll" "$out/emitted/$name.ll" || return
  mkdir -p "$work/$name.stages"
  LLRM_MIR_STAGES=$work/$name.stages "$@" -o "$work/$name.obj" >/dev/null 2>"$work/$name.compile"
  local last; last=$(ls "$work/$name.stages"/[0-9]*.ll 2>/dev/null | tail -1)
  if [ "$(basename "${last:-none}")" = "$pipeline_end" ]; then kept "$last" "$out/optimized/$name.ll"; else echo "no optimized $name: $(head -1 "$work/$name.compile")"; fi
}
qb() {
  local name=$1 source=$2; shift 2
  if "$bin/llrm-qb" "$source" "$@" --dump-hir "$work/$name.json" --mir >/dev/null 2>"$work/$name.frontend"; then
    add "$name" "$work/$name.json" "$bin/llrm-qb" "$source" "$@"
  else
    echo "frontend fails: $name"
  fi
}
nib() {
  local name=$1 source=$2
  if "$bin/llrm-nib" "$source" --dump "$work/$name.d" >/dev/null 2>"$work/$name.frontend"; then
    add "$name" "$work/$name.d/03-hir.json" "$bin/llrm-nib" "$source"
  else
    echo "frontend fails: $name: $(head -1 "$work/$name.frontend")"
  fi
}
# The pipeline's last pass, as the stage dumps name it.
mkdir "$work/probe"
LLRM_MIR_STAGES=$work/probe "$bin/llrm-qb" "$root/tests/suite/addrm.bas" -o "$work/probe.obj" >/dev/null 2>&1
pipeline_end=$(ls "$work/probe" | tail -1)
for source in "$root"/tests/suite/*.bas; do
  qb "qb-$(basename "$source" .bas)" "$source"
done
for demo in qbdemo oimad deedlines; do
  [ -f "$demos/$demo/TSC.BAS" ] && qb "demo-$demo" "$demos/$demo/TSC.BAS" --dialect qb45 --runtime qb45
done
for source in $(find "$root/tests/fixtures/nib" "$root/examples" -name '*.nib' | sort); do
  nib "nib-$(echo "${source#"$root"/}" | sed -e 's|^tests/fixtures/nib/||' -e 's|/|-|g' -e 's/\.nib$//')" "$source"
done
nib nib-runtime "$root/crates/llrm-nib/src/runtime/runtime.nib"
echo "emitted: $(ls "$out/emitted" | wc -l), optimized: $(ls "$out/optimized" | wc -l), $(du -sh "$out" | cut -f1)"
