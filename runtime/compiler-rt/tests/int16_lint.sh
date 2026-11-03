#!/bin/sh
# 16-bit int target: `1u << 27` silently overflowed there (6502 build, 2026-11-02).
# Every source must compile clean with a 16-bit int and 32-bit long.
cd "$(dirname "$0")/.." || exit 1
clang -target msp430 -fsyntax-only -Wall -Wextra -Werror -Iinclude lib/*/*.c
# The integer-ABI build (no float type in any signature) must compile too.
clang -target msp430 -fsyntax-only -Wall -Wextra -Werror -DSF_INTABI -Iinclude lib/fp/sf32.c lib/fp/sf32_conv.c lib/fp/sf32_conv64.c lib/int/int32.c lib/int/int64.c
