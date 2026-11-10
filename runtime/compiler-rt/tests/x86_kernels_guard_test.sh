#!/bin/bash
# usage: tests/x86_kernels_guard_test.sh
# x86_kernels.sh without SF_CFG wrote x_sf_* kernels with seven full compare wrappers; the m32 table read them as a +100 B compiler gap on 8 cells. It must refuse, and write nothing.
cd "$(dirname "$0")/.." || exit 2
K=../../crates/target/llrm-x86-m32/vsgcc/kernels; before=$(cat $K/x_sf_*/x_sf_*.c | md5sum)
out=$(env -u SF_CFG bash tests/x86_kernels.sh 2>&1); rc=$?
[ $rc -ne 0 ] || { echo "FAIL: x86_kernels.sh ran without SF_CFG (exit 0): $out"; exit 1; }
case $out in *SF_CFG*) ;; *) echo "FAIL: refusal does not name SF_CFG: $out"; exit 1;; esac
[ "$(cat $K/x_sf_*/x_sf_*.c | md5sum)" = "$before" ] || { echo "FAIL: kernels rewritten by a refused run"; exit 1; }
echo ok
