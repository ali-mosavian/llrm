#!/bin/sh
# 16-bit int target: `1u << 27` silently overflowed there (6502 build, 2026-11-02).
# Every source must compile clean with a 16-bit int and 32-bit long. Not lib/fp/f128: binary128 needs a 128-bit integer type, which msp430 lacks
# (this line failed on sf128 from #6797 on, 2026-11-04).
cd "$(dirname "$0")/.." || exit 1
set -e # the first clang used to fail unnoticed: only the last command set the exit status
clang -target msp430 -fsyntax-only -Wall -Wextra -Werror -Iinclude lib/*/*.c lib/fp/f32/*.c lib/fp/f64/*.c lib/fp/f8/*.c
# The integer-ABI build (no float type in any signature) must compile too.
clang -target msp430 -fsyntax-only -Wall -Wextra -Werror -DSF_INTABI -Iinclude lib/fp/f32/*.c lib/int/int32.c $(tests/fp_sources.sh int64)
