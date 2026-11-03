#!/usr/bin/env bash
# Regression test of wasm_corpus.sh: grep (it reads `input_read`) once linked with a stub that lacked it, did not
# instantiate, and its row read "0 0 same" on both sides. A program that runs must report instructions executed.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
row=$("$root/tools/wasm_corpus.sh" O1 "$root/bench/grep/grep.c" | tail -1)
read -r _ _ _ llrm_instr clang_instr result <<< "$row"
[ "$llrm_instr" -gt 0 ] && [ "$clang_instr" -gt 0 ] && [ "$result" = same ] || { echo "FAIL: $row"; exit 1; }
echo "ok: $row"
