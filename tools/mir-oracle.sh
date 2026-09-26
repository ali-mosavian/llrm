#!/usr/bin/env bash
# LLVM 20 as MIR's oracle (docs/architecture/rich-mir.md): every .ll given
# must pass opt's verifier, compile for msp430 unless it says "; msp430: no",
# and, where it says "; expect: N", exit with N under LLVM's interpreter.
# llrm-mir must read it and write it back as LLVM reads it: both texts
# disassemble alike.
set -u
bin=${LLVM20:-}
for guess in /usr/lib/llvm-20/bin /opt/homebrew/opt/llvm@20/bin; do
  [ -z "$bin" ] && [ -x "$guess/opt" ] && bin=$guess
done
if [ -z "$bin" ] || ! "$bin/opt" --version | grep -q 'version 20\.'; then
  echo "mir-oracle: LLVM 20 not found; set LLVM20 to its bin directory" >&2
  exit 2
fi
root=$(cd "$(dirname "$0")/.." && pwd)
cargo build -q --release -p llrm-mir --manifest-path "$root/Cargo.toml" || exit 2
ours=$(mktemp -d)
trap 'rm -rf "$ours"' EXIT
canonical() { "$bin/llvm-as" -o - "$1" | "$bin/llvm-dis" -o - | grep -v -e '^; ModuleID' -e '^source_filename'; }
failed=0
for file in "$@"; do
  why=""
  "$bin/opt" -passes=verify -disable-output "$file" 2>/dev/null || why="opt rejects it"
  [ -z "$why" ] && ! grep -q '^; msp430: no' "$file" && { "$bin/llc" -mtriple=msp430 -o /dev/null "$file" 2>/dev/null || why="llc msp430 rejects it"; }
  want=$(sed -n 's/^; expect: //p' "$file" | head -1)
  if [ -z "$why" ]; then
    written="$ours/$(basename "$file")"
    if ! "$root/target/release/llrm-mir" "$file" > "$written" 2> "$written.err"; then
      why="llrm-mir refuses it: $(cat "$written.err")"
    elif ! diff <(canonical "$file") <(canonical "$written") > "$written.diff" 2>&1; then
      why="llrm-mir's round trip differs: $(head -5 "$written.diff" | tr '\n' ' ')"
    fi
  fi
  if [ -z "$why" ] && [ -n "$want" ] && [ "$want" != none ]; then
    "$bin/lli" -force-interpreter "$file" >/dev/null 2>&1
    got=$?
    [ "$got" = "$want" ] || why="lli exits $got, expected $want"
  fi
  if [ -n "$why" ]; then echo "FAIL $file: $why"; failed=1; else echo "ok   $file"; fi
done
exit $failed
