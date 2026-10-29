#!/bin/sh
# Build one Nib module into a DOS executable on the host:
# llrm-nib compiles the program and crates/frontends/nib/src/runtime/runtime.nib, jwasm
# assembles, jwlink links, with any C or assembly files the program imports
# from. Those C files may include SOURCE's generated declarations as
# "NAME.h". Running it is a separate, visible DOSBox step.
#
#   [NIB_FLAGS='-fno-inline-functions -fno-inline-functions-called-once'] [NIB_MAP=LISTING.map] [NIB_OBJ=PROGRAM.obj, the program's own object kept] tools/nib-build.sh SOURCE.nib [OUTPUT.EXE] [-O2|-Os] [FOREIGN.c|.asm ...]
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
source=$1
output=${2:-${source%.*}.exe}
level=${3:--O2}
shift $(($# < 3 ? $# : 3))
bin=$(python3 "$root/tools/llrmbin.py" bin)
target=x86-m16
toolchain=${TOOLCHAIN:-$HOME/work/other/d32x/toolchains/native/bin}

# The ABI the program is built for is the runtime's and the foreign C's too: their calls cross it.
abi=""
for flag in ${NIB_FLAGS:-}; do
    case $flag in -mabi=*) abi="$abi $flag"; abi_name=${flag#-mabi=} ;; esac
done
# The instruction selector a run asks for (LLRM_ISEL=dag): the program, the foreign C and the runtime go through it.
isel=${LLRM_ISEL:+--isel=$LLRM_ISEL}
work=$(mktemp -d "${TMPDIR:-/tmp}/nib-build.XXXXXX")
trap 'rm -rf "$work"' EXIT

"$bin/llrm-nib" $isel "$source" -o "$work/program.obj" "$level" --procedure-segments ${NIB_FLAGS:-} >/dev/null
"$bin/nibfront" --declare h $abi "$source" >"$work/$(basename "$source" .nib).h"
defines=""
recipe() { python3 "$root/tools/linkrecipe.py" "$target" "$1"; }
omf=$(recipe assembler)
for one in $("$bin/llrm-nib" --os-layer defines); do defines="$defines -D$one"; done
objects=""
used="--used-by $work/program.obj"
for part in "$@"; do
    name=$(basename "$part")
    case $part in
    *.asm) "$toolchain/jwasm" -q -c -Cp -Zg $omf $defines "-Fo$work/$name.obj" "$part" ;;
    # llrm-cc compiles the foreign C by clang (LLRM_CLANG_M16 names the i8086 build).
    *) LLRM_CC_TARGET=x86_16-dos "$bin/llrm-cc" -c "$part" -I "$work" -o "$work/$name.obj" "$level" -mabi="${abi_name:-regparm3}" >/dev/null ;;
    esac
    objects="$objects file $work/$name.obj"
    used="$used --used-by $work/$name.obj"
done
# The OS layer's objects come from the one function that knows which they are (dosbatch.os_objects).
for line in $(python3 "$root/tools/dosbatch/os_objects.py" "$target" nib "$work"); do
    field=${line%%=*}
    used="$used --used-by ${line#*=}"
    case $field in hook) hookobj="file ${line#*=}" ;; esac
done
# jwlink keeps whatever any segment references, even one it drops, so the
# runtime keeps only the routines the other objects name.
"$bin/llrm-nib" $isel "$root/crates/frontends/nib/src/runtime/runtime.nib" -o "$work/runtime.obj" "$level" --procedure-segments $abi $used >/dev/null
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
