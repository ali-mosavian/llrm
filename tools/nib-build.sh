#!/bin/sh
# Build one Nib module into a DOS executable on the host:
# llrm-nib compiles the program and crates/frontends/llrm-nib/src/runtime/runtime.nib, jwasm
# assembles, jwlink links, with any C or assembly files the program imports
# from. Those C files may include SOURCE's generated declarations as
# "NAME.h". Running it is a separate, visible DOSBox step.
#
#   [NIB_FLAGS='-fno-inline-functions'] [NIB_MAP=LISTING.map] [NIB_OBJ=PROGRAM.obj, the program's own object kept] tools/nib-build.sh SOURCE.nib [OUTPUT.EXE] [-O2|-Os] [FOREIGN.c|.asm ...]
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
source=$1
output=${2:-${source%.*}.exe}
level=${3:--O2}
shift $(($# < 3 ? $# : 3))
bin=$(python3 "$root/tools/llrmbin.py" bin)
toolchain=${TOOLCHAIN:-$HOME/work/other/d32x/toolchains/native/bin}

work=$(mktemp -d "${TMPDIR:-/tmp}/nib-build.XXXXXX")
trap 'rm -rf "$work"' EXIT

"$bin/llrm-nib" "$source" -o "$work/program.obj" "$level" --procedure-segments ${NIB_FLAGS:-} >/dev/null
"$bin/nibfront" --declare h "$source" >"$work/$(basename "$source" .nib).h"
defines=""
recipe() { python3 "$root/tools/linkrecipe.py" x86-m16 "$1"; }
omf=$(recipe assembler)
for one in $("$bin/llrm-nib" --os-layer defines); do defines="$defines -D$one"; done
objects=""
used="--used-by $work/program.obj"
for part in "$@"; do
    name=$(basename "$part")
    case $part in
    *.asm) "$toolchain/jwasm" -q -c -Cp -Zg $omf $defines "-Fo$work/$name.obj" "$part" ;;
    *) "$bin/llrm-c" "$part" -I "$work" -o "$work/$name.obj" "$level" >/dev/null ;;
    esac
    objects="$objects file $work/$name.obj"
    used="$used --used-by $work/$name.obj"
done
layer=$("$bin/llrm-nib" --os-layer directory)
for field in start implementation; do
    part=$("$bin/llrm-nib" --os-layer $field)
    # shellcheck disable=SC2086
    "$toolchain/jwasm" -q -c -Cp -Zg $omf $defines "-Fo$work/$field.obj" "$layer/$part"
    used="$used --used-by $work/$field.obj"
done
hook=$("$bin/llrm-nib" --os-layer language_file)
if [ -n "$hook" ]; then
    # shellcheck disable=SC2086
    "$toolchain/jwasm" -q -c -Cp -Zg $omf $defines "-Fo$work/hook.obj" "$hook"
    used="$used --used-by $work/hook.obj"
    hookobj="file $work/hook.obj"
fi
# jwlink keeps whatever any segment references, even one it drops, so the
# runtime keeps only the routines the other objects name.
"$bin/llrm-nib" "$root/crates/frontends/llrm-nib/src/runtime/runtime.nib" -o "$work/runtime.obj" "$level" --procedure-segments $used >/dev/null
objects="file $work/runtime.obj$objects"
"$toolchain/jwlink" option quiet option eliminate ${NIB_MAP:+option map=$NIB_MAP} $(recipe format) name "$work/program.exe" \
    file "$work/start.obj" ${hookobj:-} file "$work/program.obj" $objects \
    file "$work/implementation.obj" >"$work/link.out" || {
    cat "$work/link.out" >&2
    exit 1
}
cp "$work/program.exe" "$output"
[ -z "${NIB_OBJ:-}" ] || cp "$work/program.obj" "$NIB_OBJ"
echo "$output ($(wc -c <"$output" | tr -d ' ') bytes)"
