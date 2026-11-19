#!/bin/sh
# Open Watcom at the commit toolchain/owshim/ow-commit pins, its C library built, in ~/.cache/llrm/ow-src-<commit>: the tree llrm-cc's {ow} names.
# Run by hand or by CI; llrm-cc and the tests never run it. It prints what it clones, then clones and builds (toolchain/owclib.sh); prints the directory.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
echo "ow-fetch: git clone https://github.com/open-watcom/open-watcom-v2.git at $(cat "$here/../toolchain/owshim/ow-commit"), then build its C library" >&2
exec sh "$here/../toolchain/owclib.sh" "$@"
