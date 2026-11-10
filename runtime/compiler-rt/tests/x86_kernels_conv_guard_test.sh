#!/bin/bash
# usage: tests/x86_kernels_conv_guard_test.sh
# x86_kernels_conv.sh without SF_CFG exited 0 and wrote x_{sf,crt,sfp}_{cmp,floatsi,fixsi} with seven full compare wrappers. It must refuse, and write nothing.
cd "$(dirname "$0")/.." || exit 2
K=../../crates/target/llrm-x86-m32/vsgcc/kernels; before=$(ls $K | md5sum)
out=$(env -u SF_CFG bash tests/x86_kernels_conv.sh 2>&1); rc=$?
after=$(ls $K | md5sum)
[ $rc -ne 0 ] || { echo "FAIL: x86_kernels_conv.sh ran without SF_CFG (exit 0)"; exit 1; }
case $out in *SF_CFG*) ;; *) echo "FAIL: refusal does not name SF_CFG: $out"; exit 1;; esac
[ "$before" = "$after" ] || { echo "FAIL: a refused run wrote kernels"; exit 1; }
echo ok
