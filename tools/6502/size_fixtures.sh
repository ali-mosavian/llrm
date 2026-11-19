#!/bin/bash
# The front-end IR of the pass82 programs, for crates/target/llrm-6502/tests/size_calibration.rs.
# usage: size_fixtures.sh DIST_LLRM_CC OUTDIR
cc=$1; out=$2; mkdir -p "$out"
for n in $(cat "$(dirname "$0")/pass82.txt"); do
  LLRM_CC_TARGET=6502-cc65 "$cc" -S -emit-llvm -Os -Xclang -disable-llvm-passes "$(dirname "$0")/../../tests/run/c/$n.c" -o "$out/$n.ll" >/dev/null 2>&1 || { echo "skip $n" >&2; rm -f "$out/$n.ll"; continue; }
  sed -i -e '/^target datalayout/d' -e '/^; ModuleID/d' -e '/^source_filename/d' "$out/$n.ll"
done
