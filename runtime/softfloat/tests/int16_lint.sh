#!/bin/sh
# 16-bit int target: `1u << 27` silently overflowed there (6502 build, 2026-11-02).
# Every source must compile clean with a 16-bit int and 32-bit long.
cd "$(dirname "$0")/.." || exit 1
clang -target msp430 -fsyntax-only -Wall -Wextra -Werror -Iinclude src/*.c
