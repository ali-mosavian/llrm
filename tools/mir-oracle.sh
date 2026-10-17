#!/usr/bin/env bash
# LLVM 20 as MIR's oracle (docs/architecture/rich-mir.md): every .ll given
# must pass opt's verifier, compile for msp430 unless it says "; msp430: no",
# and, where it says "; expect: N", exit with N under LLVM's interpreter
# and llrm's alike.
# llrm-mir must read it and write it back as LLVM reads it: both texts
# disassemble alike. A file saying "; invalid: REASON" must instead be
# refused by opt, and by llrm-mir for REASON.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
# The pinned LLVM (tools/llvmpin.py): llrm-mir's text is read by the opt of the build the plan is judged against, not the distro's.
bin=$(python3 "$root/tools/llvmpin.py" dir)
[ -x "$bin/opt" ] || { echo "mir-oracle: the pinned LLVM is not built at $bin" >&2; exit 2; }
echo "mir-oracle: $("$bin/opt" --version | sed -n 2p) ($bin)" >&2
llrm_bin=$(python3 "$root/tools/llrmbin.py" bin)
cargo build -q --release --bin llrm-mir --manifest-path "$root/Cargo.toml" || exit 2
ours=$(mktemp -d)
trap 'rm -rf "$ours"' EXIT
canonical() { "$bin/llvm-as" -o - "$1" | "$bin/llvm-dis" -o - | grep -v -e '^; ModuleID' -e '^source_filename'; }
failed=0
for file in "$@"; do
  why=""
  reason=$(sed -n 's/^; invalid: //p' "$file" | head -1)
  if [ -n "$reason" ]; then
    "$bin/opt" -passes=verify -disable-output "$file" 2>/dev/null && why="opt accepts it"
    if [ -z "$why" ] && ! "$llrm_bin/llrm-mir" "$file" 2>&1 >/dev/null | grep -qF "$reason"; then
      why="llrm-mir does not refuse it for: $reason"
    fi
    if [ -n "$why" ]; then echo "FAIL $file: $why"; failed=1; else echo "ok   $file (refused)"; fi
    continue
  fi
  "$bin/opt" -passes=verify -disable-output "$file" 2>/dev/null || why="opt rejects it"
  [ -z "$why" ] && ! grep -q '^; msp430: no' "$file" && { "$bin/llc" -mtriple=msp430 -o /dev/null "$file" 2>/dev/null || why="llc msp430 rejects it"; }
  want=$(sed -n 's/^; expect: //p' "$file" | head -1)
  if [ -z "$why" ]; then
    written="$ours/$(basename "$file")"
    if ! "$llrm_bin/llrm-mir" "$file" > "$written" 2> "$written.err"; then
      why="llrm-mir refuses it: $(cat "$written.err")"
    elif ! diff <(canonical "$file") <(canonical "$written") > "$written.diff" 2>&1; then
      why="llrm-mir's round trip differs: $(head -5 "$written.diff" | tr '\n' ' ')"
    fi
  fi
  if [ -z "$why" ] && [ -n "$want" ] && [ "$want" != none ]; then
    "$bin/lli" -force-interpreter "$file" >/dev/null 2>&1
    got=$?
    [ "$got" = "$want" ] || why="lli exits $got, expected $want"
    if [ -z "$why" ]; then
      ran=$("$llrm_bin/llrm-mir" --run "$file" 2>&1)
      [ "$ran" = "${ran//[^0-9]/}" ] && [ -n "$ran" ] && [ $((ran % 256)) = "$want" ] || why="llrm-mir runs it to $ran, expected $want"
    fi
  fi
  if [ -n "$why" ]; then echo "FAIL $file: $why"; failed=1; else echo "ok   $file"; fi
done
exit $failed
