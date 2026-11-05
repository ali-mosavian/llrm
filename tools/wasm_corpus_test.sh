#!/usr/bin/env bash
# Regression tests of wasm_corpus.sh.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)

# grep (it reads `input_read`) once linked with a stub that lacked it, did not instantiate, and its row read
# "0 0 same" on both sides. A program that runs must report instructions executed.
row=$("$root/tools/wasm_corpus.sh" O1 "$root/bench/grep/grep.c" | tail -1)
read -r _ _ _ llrm_instr clang_instr _ _ result <<< "$row"
[ "$llrm_instr" -gt 0 ] && [ "$clang_instr" -gt 0 ] && [ "$result" = same ] || { echo "FAIL: $row"; exit 1; }
echo "ok: $row"

# Bytes were the Code section alone, so the data a program loads (switch's lookup table: 35 B on our side to the
# pin's 18) never reached a cell. llrm_B/clang_B are Code + Data, the last two columns Code alone.
row=$("$root/tools/wasm_corpus.sh" O2 "$root/bench/switch/switch.c" | tail -1)
read -r _ llrm_b clang_b _ _ llrm_code clang_code _ <<< "$row"
[ "$clang_b" -gt "$clang_code" ] && [ "$llrm_b" -ge "$llrm_code" ] || { echo "FAIL: bytes lack the Data section: $row"; exit 1; }
echo "ok: $row"
