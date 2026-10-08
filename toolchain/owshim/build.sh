#!/bin/sh
# Relink Open Watcom's C front end against cgshim.c instead of its code
# generator, as wccq in the directory given (default toolchain/owshim/bin).
# OWCPU picks the front end: i86 (16-bit, the default) or 386 (flat 32-bit,
# default directory bin386).
#
# Run by build.rs. OWROOT is an Open Watcom tree: the user's own when set,
# else one cached per commit (ow-commit) under ~/.cache/llrm, cloned and
# bootstrapped once, whole or not at all (cache.sh).
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
. "$HERE/../cache.sh"
OW_COMMIT=$(cat "$HERE/ow-commit")
ow_tree() {
    git clone -q --filter=blob:none https://github.com/open-watcom/open-watcom-v2.git "$1"
    git -C "$1" checkout -q "$OW_COMMIT"
    ( set +u; cd "$1" && . ./setvars.sh && ./build.sh boot )
}
if [ -z "${OWROOT:-}" ]; then
    OWROOT="${XDG_CACHE_HOME:-$HOME/.cache}/llrm/open-watcom-v2-$OW_COMMIT"
    cached "$OWROOT" ow_tree
elif [ ! -x "$OWROOT/build/binbuild/wmake" ] || [ ! -f "$OWROOT/bld/cc/${OWCPU:-i86}/binbuild/ccheck.obj" ]; then
    ( set +u; cd "$OWROOT" && . ./setvars.sh && ./build.sh boot )
fi
CPU="${OWCPU:-i86}"
case "$CPU" in
    i86) BIN=bin ;;
    386) BIN=bin386 ;;
    *) echo "OWCPU is i86 or 386, not $CPU" >&2; exit 1 ;;
esac
# What the target's description says, which build.rs reads for each Open Watcom tree (FLAT: 1 where far is near).
FLAT=${LLRM_FLAT:?LLRM_FLAT comes from the target description}
NEAR=${LLRM_NEAR_BYTES:?LLRM_NEAR_BYTES comes from the target description}
FAR=${LLRM_FAR_BYTES:?LLRM_FAR_BYTES comes from the target description}
INT=${LLRM_INT_BYTES:?LLRM_INT_BYTES comes from the target description}
CC_OBJ="$OWROOT/bld/cc/$CPU/binbuild"
OUT="${1:-$HERE/$BIN}"
mkdir -p "$OUT"
STAMP=$(OWCPU=$CPU "$HERE/hash.sh")
if [ -x "$OUT/wccq" ] && [ "$(cat "$OUT/stamp" 2>/dev/null)" = "$STAMP" ]; then
    echo "$OUT/wccq"
    exit 0
fi
rm -f "$OUT/stamp"

# cc's own compile line from a forced dry run, so every object here sees the
# host configuration and include path the front end was built with.
CC_LINE=$(set +u; cd "$OWROOT" && . ./setvars.sh >/dev/null && cd "$CC_OBJ" \
    && "$OWROOT/build/binbuild/wmake" -h -a -n -f ../binmake bootstrap=1 \
    | grep -- '-o ccheck.obj' | sed -e 's|"||g' -e 's| -o ccheck.obj||' -e 's| [^ ]*/ccheck\.c$||')
[ -n "$CC_LINE" ] || { echo "no compile line for ccheck.obj in $CC_OBJ" >&2; exit 1; }

compile() { ( cd "$CC_OBJ" && $CC_LINE -I"$HERE" -DLLRM_FLAT=$FLAT -DLLRM_NEAR_BYTES=$NEAR -DLLRM_FAR_BYTES=$FAR -DLLRM_INT_BYTES=$INT -o "$1" "$2" ); }

compile "$OUT/cgshim.o" "$HERE/cgshim.c"
compile "$OUT/i64.o" "$OWROOT/bld/watcom/c/i64.c"
rm -f "$OUT/libcgshim.a"
ar rcs "$OUT/libcgshim.a" "$OUT/cgshim.o" "$OUT/i64.o"

# Where Open Watcom's C dialect is not Borland's (patches/), the front end's
# own source with the patch applied.
# The two objects named for the CPU: codei86/pragi86, code386/prag386.
OBJS=$(sed "s/codei86/code$CPU/;s/pragi86/prag$CPU/" "$HERE/cc-objects.txt")
PATCHED="$OUT/patched"
rm -rf "$PATCHED" && mkdir -p "$PATCHED"
for patch in "$HERE"/patches/*.patch; do
    source=$(basename "$patch" .patch)
    cp "$OWROOT/bld/cc/c/$source" "$PATCHED/$source"
    patch -s "$PATCHED/$source" "$patch"
    object="${source%.c}.obj"
    compile "$PATCHED/$object" "$PATCHED/$source"
    OBJS=$(echo "$OBJS" | tr ' ' '\n' | sed "s|^$object\$|$PATCHED/$object|" | tr '\n' ' ')
done

# Every object bwcc links (cc-objects.txt), without the code generator's libraries.
( cd "$CC_OBJ" && ${CC:-cc} -pipe -o "$OUT/wccq" $OBJS "$OUT/libcgshim.a" \
    "$OWROOT/bld/cfloat/binbuild/cf.lib" \
    "$OWROOT/bld/dwarf/dw/binbuild/dwarfw.lib" \
    "$OWROOT/bld/watcom/binbuild/clibext.lib" )
echo "$STAMP" >"$OUT/stamp"
echo "$OUT/wccq"
