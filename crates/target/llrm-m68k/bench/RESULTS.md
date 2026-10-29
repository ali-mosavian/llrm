# Results, 2026-10-29 11:28 (pin d1106deb71cc, qemu-m68k -cpu m68000)

| kernel | ours B | -O2 B | -Os B | ours insns | -O2 insns | -Os insns | result |
|---|---|---|---|---|---|---|---|
| sum | 46 | 46 | 46 | 48 | 48 | 48 | 45 all |
| popcount | 52 | 52 | 52 | 103 | 103 | 103 | 8 all |
| max | 16 | 16 | 16 | 6 | 6 | 6 | 42 all |
| absdiff | 22 | 22 | 22 | 7 | 7 | 7 | 42 all |
| clamp | 26 | 26 | 26 | 9 | 9 | 9 | 42 all |
| fib | 54 | 66 | 66 | 79 | 139 | 139 | 89 all |
| gcd | 58 | 32 | 26 | 31 | 16 | 17 | 42 all |
| arrsum | 56 | 64 | 64 | 105 | 153 | 153 | 80 all |
| arrmax | 64 | 92 | 88 | 156 | 248 | 235 | 9 all |
| countgt | 78 | 86 | 86 | 186 | 234 | 234 | 9 all |
| strlen | 26 | 36 | 36 | 73 | 89 | 89 | 13 all |
| copy | 64 | 42 | 42 | 76 | 59 | 59 | 33 all |

Equal to llc: sum, popcount, max, absdiff, clamp. Smaller and faster: fib, arrsum, arrmax, countgt, strlen.
Bigger and slower than llc: gcd (58 B vs 32/26; 31 vs 16 insns), copy (64 B vs 42; 76 vs 59 insns).
