#!/bin/sh
# Relink Open Watcom's 16-bit C front end against cgshim.c instead of its
# code generator, as owshim/bin/wccq.
#
# Needs a bootstrapped OW tree ($OWROOT, default ~/work/open-watcom-v2):
#   OWTOOLS=CLANG ./build.sh boot
# which leaves cc's objects in bld/cc/i86/binbuild.
set -eu

OWROOT="${OWROOT:-$HOME/work/open-watcom-v2}"
HERE="$(cd "$(dirname "$0")" && pwd)"
CC_OBJ="$OWROOT/bld/cc/i86/binbuild"
OUT="$HERE/bin"
mkdir -p "$OUT"

# cc's own flags (wmake -n -f bld/cc/i86/binmake bootstrap=1), so the headers
# see the same configuration the front end was compiled with.
FLAGS="-pipe -c -std=gnu99 -O -DNDEBUG -DBOOTSTRAP -D__UNIX__ -D__FLAT__ -D_M_ARM64 -D__OSX_ARM64__ -D__OSX__
  -fno-asm -fno-common -fsigned-char -Wall -Wno-switch -Wno-missing-braces
  -Werror=implicit-function-declaration
  -I$OWROOT/bld/cg/intel/i86/h -I$OWROOT/bld/cg/intel/h -I$OWROOT/bld/cg/h
  -I$OWROOT/bld/fe_misc/h -I$OWROOT/bld/watcom/h"

clang $FLAGS -o "$OUT/cgshim.o" "$HERE/cgshim.c"
clang $FLAGS -o "$OUT/i64.o" "$OWROOT/bld/watcom/c/i64.c"
rm -f "$OUT/libcgshim.a"
ar rcs "$OUT/libcgshim.a" "$OUT/cgshim.o" "$OUT/i64.o"

# Where Open Watcom's C dialect is not Borland's (patches/), the front end's
# own source with the patch applied, built with cc's flags from the same dry run.
CC_FLAGS="-pipe -c -std=gnu99 -O -D_BLDVER=1300 -D_CYEAR=2026 -DNDEBUG -DBOOTSTRAP -DINCL_MSGTEXT
  -D__UNIX__ -D__FLAT__ -D_M_ARM64 -D__OSX_ARM64__ -D__OSX__ -DIDE_PGM -fno-asm -fno-common -fsigned-char
  -Wno-switch -Wno-missing-braces -Wno-parentheses -Werror=implicit-function-declaration
  -I$CC_OBJ -I$OWROOT/bld/cc/i86 -I$OWROOT/bld/cc/h -I$OWROOT/bld/cg/intel/i86/h -I$OWROOT/bld/cg/intel/h
  -I$OWROOT/bld/cg/h -I$OWROOT/bld/wasm/h -I$OWROOT/bld/owl/h -I$OWROOT/bld/dwarf/dw/h
  -I$OWROOT/bld/comp_cfg/h -I$OWROOT/bld/fe_misc/h -I$OWROOT/bld/watcom/h"
OBJS=$(cat "$HERE/cc-objects.txt")
PATCHED="$OUT/patched"
rm -rf "$PATCHED" && mkdir -p "$PATCHED"
for patch in "$HERE"/patches/*.patch; do
    source=$(basename "$patch" .patch)
    cp "$OWROOT/bld/cc/c/$source" "$PATCHED/$source"
    patch -s "$PATCHED/$source" "$patch"
    object="${source%.c}.obj"
    clang $CC_FLAGS -o "$PATCHED/$object" "$PATCHED/$source"
    OBJS=$(echo "$OBJS" | tr ' ' '\n' | sed "s|^$object\$|$PATCHED/$object|" | tr '\n' ' ')
done

# Every object bwcc links (cc-objects.txt, from the same dry run), without
# cgi86.lib and cgi86osx.lib.
( cd "$CC_OBJ" && clang -pipe -o "$OUT/wccq" $OBJS "$OUT/libcgshim.a" \
    "$OWROOT/bld/cfloat/binbuild/cf.lib" \
    "$OWROOT/bld/dwarf/dw/binbuild/dwarfw.lib" \
    "$OWROOT/bld/watcom/binbuild/clibext.lib" )
echo "$OUT/wccq"
