#!/usr/bin/env bash
# tools/targets/6502-link.sh TARGET OUT OBJECT...: a 6502 program for cc65's sim6502 (sim65): the ca65 objects, cc65's startup and library,
# the soft-stack constructor, and the runtime as an archive of per-routine objects (tools/6502/mkrt.py), so ld65 pulls the members the
# program calls. The archive is built once per compiler and cached. TARGET is the llrm-cc row without its platform: 6502 or 65c02.
# LLRM_6502_MAP=file writes the link map.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
cpu=$1; out=$2; shift 2
cc=${LLRM_CC:?llrm-cc names itself in LLRM_CC when it links}
cache=${XDG_CACHE_HOME:-$HOME/.cache}/llrm-6502-rt/$cpu
mkdir -p "$cache"
lib=$cache/rt.lib
if [ ! -s "$lib" ] || [ "$lib" -ot "$cc" ] || [ "$lib" -ot "$root/crates/target/llrm-6502/runtime/mos_rt.s" ] || [ "$lib" -ot "$root/tools/6502/mkrt.py" ]; then
  python3 -I "$root/tools/6502/mkrt.py" "$cc" "$lib.$$" "$cache/work.$$" "--target=$cpu-sim65" >/dev/null
  mv "$lib.$$" "$lib"; rm -rf "$cache/work.$$"
fi
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
python3 -I "$root/tools/6502/mkrt.py" --startup > "$work/start.s"
map=()
[ -z "${LLRM_6502_MAP:-}" ] || map=(-m "$LLRM_6502_MAP")
exec cl65 -t sim6502 "${map[@]}" -o "$out" "$work/start.s" "$@" "$lib"
