#!/bin/bash
# Build llrm-cc and llrm-llc with profile-guided optimization on the dist profile; the objects they write are byte-identical to the plain dist's.
#
#   tools/pgo_dist.sh [--check] [OUTDIR]     OUTDIR default: $PGO_WORK/bin (PGO_WORK default: ~/pgo-dist)
#
# 1. build the dist profile with -Cprofile-generate     (target dir $PGO_WORK/gen)
# 2. run tools/pgo_train.py with that llrm-cc            (tests/run/c minus the files the gain is measured on, -O1/-O2/-Os)
# 3. merge the raw profiles with rustc's own llvm-profdata
# 4. build the dist profile with -Cprofile-use           (target dir $PGO_WORK/use) and copy both binaries to OUTDIR
# Measured (x86_32, 9 files): instructions -16..-19%, cycles -5..-13%, build +10 minutes. Remove $PGO_WORK afterwards: it holds several GB.
# `--check` builds nothing: it checks that rustc's llvm-profdata and rustc agree on the LLVM version (a profile from another LLVM is refused
# at -Cprofile-use) and that the training list exists.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=${PGO_WORK:-$HOME/pgo-dist}

sysroot=$(rustc --print sysroot)
host=$(rustc -vV | sed -n 's/^host: //p')
profdata="$sysroot/lib/rustlib/$host/bin/llvm-profdata"
[ -x "$profdata" ] || { echo "pgo_dist: no llvm-profdata at $profdata: rustup component add llvm-tools" >&2; exit 1; }
rustc_llvm=$(rustc -vV | sed -n 's/^LLVM version: //p')
data_llvm=$("$profdata" --version | sed -n 's/.*LLVM version \([0-9.]*\).*/\1/p' | head -1)
[ "${rustc_llvm%%.*}" = "${data_llvm%%.*}" ] || { echo "pgo_dist: rustc is LLVM $rustc_llvm, its llvm-profdata is $data_llvm" >&2; exit 1; }
training=$(python3 "$ROOT/tools/pgo_train.py" --list | wc -l)
[ "$training" -gt 0 ] || { echo "pgo_dist: no training programs under tests/run/c" >&2; exit 1; }

if [ "${1:-}" = "--check" ]; then
    echo "pgo_dist: ok (LLVM $rustc_llvm, $training training programs)"
    exit 0
fi

OUT=${1:-$WORK/bin}
export CARGO_PROFILE_DIST_DEBUG=line-tables-only
bins=(--bin llrm-cc --bin llrm-llc)
rm -rf "$WORK/raw" && mkdir -p "$WORK/raw" "$OUT"

(cd "$ROOT" && RUSTFLAGS="-Cprofile-generate=$WORK/raw" CARGO_TARGET_DIR="$WORK/gen" cargo build --profile dist -p llrm "${bins[@]}")
LLVM_PROFILE_FILE="$WORK/raw/p-%4m.profraw" python3 "$ROOT/tools/pgo_train.py" "$WORK/gen/dist/llrm-cc"
"$profdata" merge -o "$WORK/merged.profdata" "$WORK"/raw/*.profraw
(cd "$ROOT" && RUSTFLAGS="-Cprofile-use=$WORK/merged.profdata -Cllvm-args=-pgo-warn-missing-function=false" CARGO_TARGET_DIR="$WORK/use" cargo build --profile dist -p llrm "${bins[@]}")
cp "$WORK/use/dist/llrm-cc" "$WORK/use/dist/llrm-llc" "$OUT/"
echo "pgo_dist: $OUT/llrm-cc and $OUT/llrm-llc"
