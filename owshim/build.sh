#!/bin/sh
# Relink Open Watcom's 16-bit C front end against cgshim.c instead of its
# code generator, as wccq in the directory given (default owshim/bin).
#
# Run by build.rs. OWROOT is an Open Watcom tree, cloned at OW_COMMIT and
# bootstrapped here if absent.
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
OW_COMMIT=703e1ae2f9a621dda2d28fb39b12d0d6d2788af6
OWROOT="${OWROOT:-${XDG_CACHE_HOME:-$HOME/.cache}/llrm/open-watcom-v2}"
if [ ! -d "$OWROOT/.git" ]; then
    git clone -q --filter=blob:none https://github.com/open-watcom/open-watcom-v2.git "$OWROOT"
    git -C "$OWROOT" checkout -q "$OW_COMMIT"
fi
if [ ! -x "$OWROOT/build/binbuild/wmake" ] || [ ! -f "$OWROOT/bld/cc/i86/binbuild/ccheck.obj" ]; then
    ( set +u; cd "$OWROOT" && . ./setvars.sh && ./build.sh boot )
fi
CC_OBJ="$OWROOT/bld/cc/i86/binbuild"
OUT="${1:-$HERE/bin}"
mkdir -p "$OUT"

# cc's own compile line from a forced dry run, so every object here sees the
# host configuration and include path the front end was built with.
CC_LINE=$(set +u; cd "$OWROOT" && . ./setvars.sh >/dev/null && cd "$CC_OBJ" \
    && "$OWROOT/build/binbuild/wmake" -h -a -n -f ../binmake bootstrap=1 \
    | grep -- '-o ccheck.obj' | sed -e 's|"||g' -e 's| -o ccheck.obj||' -e 's| [^ ]*/ccheck\.c$||')
[ -n "$CC_LINE" ] || { echo "no compile line for ccheck.obj in $CC_OBJ" >&2; exit 1; }

compile() { ( cd "$CC_OBJ" && $CC_LINE -o "$1" "$2" ); }

compile "$OUT/cgshim.o" "$HERE/cgshim.c"
compile "$OUT/i64.o" "$OWROOT/bld/watcom/c/i64.c"
rm -f "$OUT/libcgshim.a"
ar rcs "$OUT/libcgshim.a" "$OUT/cgshim.o" "$OUT/i64.o"

# Where Open Watcom's C dialect is not Borland's (patches/), the front end's
# own source with the patch applied.
OBJS=$(cat "$HERE/cc-objects.txt")
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
echo "$OUT/wccq"
