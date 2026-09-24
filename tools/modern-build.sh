#!/bin/sh
# Build one modern-language module into a DOS executable on the host:
# llrm-modern and llrm-c compile, jwasm assembles, jwlink links.
# Every runtime/modern/*.c is linked, and any C or assembly files the
# program imports from; jwlink keeps what is referenced. Those C files may
# include SOURCE's generated declarations as "NAME.h". Running it is a separate, visible
# DOSBox step.
#
#   tools/modern-build.sh SOURCE.mod [OUTPUT.EXE] [-O2|-Os] [FOREIGN.c|.asm ...]
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
source=$1
output=${2:-${source%.*}.exe}
level=${3:--O2}
shift $(($# < 3 ? $# : 3))
toolchain=${TOOLCHAIN:-$HOME/work/other/d32x/toolchains/native/bin}
bin=$root/target/release

work=$(mktemp -d "${TMPDIR:-/tmp}/modern-build.XXXXXX")
trap 'rm -rf "$work"' EXIT

"$bin/llrm-modern" "$source" -o "$work/program.obj" "$level" >/dev/null
"$bin/modernfront" --declare h "$source" >"$work/$(basename "$source" .mod).h"
objects=""
for part in "$root"/runtime/modern/*.c "$@"; do
    name=$(basename "$part")
    case $part in
    *.asm) "$toolchain/jwasm" -q -c -Cp -Zg -omf "-Fo$work/$name.obj" "$part" ;;
    *) "$bin/llrm-c" "$part" -I "$work" -o "$work/$name.obj" --opt "$level" >/dev/null ;;
    esac
    objects="$objects file $work/$name.obj"
done
for part in start dos; do
    "$toolchain/jwasm" -q -c -Cp -Zg -omf "-Fo$work/$part.obj" "$root/runtime/modern/$part.asm"
done
"$toolchain/jwlink" option quiet format dos name "$work/program.exe" \
    file "$work/start.obj" file "$work/program.obj" $objects \
    file "$work/dos.obj" >"$work/link.out" || {
    cat "$work/link.out" >&2
    exit 1
}
cp "$work/program.exe" "$output"
echo "$output ($(wc -c <"$output" | tr -d ' ') bytes)"
