#!/bin/sh
# Build one Nib module into a DOS executable on the host:
# llrm-nib compiles the program and runtime/nib/runtime.nbl, jwasm
# assembles, jwlink links, with any C or assembly files the program imports
# from. Those C files may include SOURCE's generated declarations as
# "NAME.h". Running it is a separate, visible DOSBox step.
#
#   tools/nib-build.sh SOURCE.nbl [OUTPUT.EXE] [-O2|-Os] [FOREIGN.c|.asm ...]
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
source=$1
output=${2:-${source%.*}.exe}
level=${3:--O2}
shift $(($# < 3 ? $# : 3))
toolchain=${TOOLCHAIN:-$HOME/work/other/d32x/toolchains/native/bin}
bin=$root/target/release

work=$(mktemp -d "${TMPDIR:-/tmp}/nib-build.XXXXXX")
trap 'rm -rf "$work"' EXIT

"$bin/llrm-nib" "$source" -o "$work/program.obj" "$level" --procedure-segments >/dev/null
"$bin/nibfront" --declare h "$source" >"$work/$(basename "$source" .mod).h"
objects=""
used="--used-by $work/program.obj"
for part in "$@"; do
    name=$(basename "$part")
    case $part in
    *.asm) "$toolchain/jwasm" -q -c -Cp -Zg -omf "-Fo$work/$name.obj" "$part" ;;
    *) "$bin/llrm-c" "$part" -I "$work" -o "$work/$name.obj" --opt "$level" >/dev/null ;;
    esac
    objects="$objects file $work/$name.obj"
    used="$used --used-by $work/$name.obj"
done
for part in start dos; do
    "$toolchain/jwasm" -q -c -Cp -Zg -omf "-Fo$work/$part.obj" "$root/runtime/nib/$part.asm"
    used="$used --used-by $work/$part.obj"
done
# jwlink keeps whatever any segment references, even one it drops, so the
# runtime keeps only the routines the other objects name.
"$bin/llrm-nib" "$root/runtime/nib/runtime.nbl" -o "$work/runtime.obj" "$level" --procedure-segments $used >/dev/null
objects="file $work/runtime.obj$objects"
"$toolchain/jwlink" option quiet option eliminate format dos name "$work/program.exe" \
    file "$work/start.obj" file "$work/program.obj" $objects \
    file "$work/dos.obj" >"$work/link.out" || {
    cat "$work/link.out" >&2
    exit 1
}
cp "$work/program.exe" "$output"
echo "$output ($(wc -c <"$output" | tr -d ' ') bytes)"
