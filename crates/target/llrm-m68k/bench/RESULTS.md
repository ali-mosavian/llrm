# Results, 2026-10-29 11:37 (pin d1106deb71cc, qemu-m68k -cpu m68000)

gcc is m68k-linux-gnu-gcc 15 (`-mcpu=68000`), the Debian cross toolchain unpacked without root. Cycles are not
modelled: "insns" are executed instructions, which price a 68000 `move` and a `muls` the same.

| kernel | ours B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | ours insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |
|---|---|---|---|---|---|---|---|---|---|---|---|
| sum | 46 | 46 | 46 | 16 | 18 | 48 | 48 | 48 | 43 | 44 | 45 all |
| popcount | 52 | 52 | 52 | 22 | 22 | 103 | 103 | 103 | 85 | 85 | 8 all |
| max | 16 | 16 | 16 | 16 | 16 | 6 | 6 | 6 | 5 | 5 | 42 all |
| absdiff | 22 | 22 | 22 | 24 | 24 | 7 | 7 | 7 | 8 | 7 | 42 all |
| clamp | 26 | 26 | 26 | 26 | 26 | 9 | 9 | 9 | 9 | 9 | 42 all |
| fib | 54 | 66 | 66 | 22 | 28 | 79 | 139 | 139 | 64 | 67 | 89 all |
| gcd | 26 B / 16 llc -O2 | 26 / 17 | one insn: llc rotates the loop (tail duplication, block placement) |
| arrsum | 56 | 64 | 64 | 20 | 22 | 105 | 153 | 153 | 68 | 69 | 80 all |
| arrmax | 64 | 92 | 88 | 26 | 32 | 156 | 248 | 235 | 97 | 100 | 9 all |
| countgt | 78 | 86 | 86 | 42 | 42 | 186 | 234 | 234 | 168 | 123 | 9 all |
| strlen | 26 | 36 | 36 | 18 | 20 | 73 | 89 | 89 | 57 | 44 | 13 all |
| copy | 38 | 42 | 42 | 34 | 34 | 59 | 59 | 59 | 59 | 59 | 33 all |

## Above llc -Os or gcc -Os (bytes / insns)

| kernel | best reference | ours | gap |
|---|---|---|---|
| sum | 16 B gcc / 43 gcc | 46 / 48 | callee-saved `d2` kept for `n` (movem + suba/adda); `add.l #1` for `addq` |
| popcount | 22 B / 85 gcc | 52 / 103 | not yet diagnosed |
| max | 16 B / 5 gcc | 16 / 6 | one insn |
| fib | 22 B / 64 gcc | 54 / 79 | not yet diagnosed |
| gcd | 26 B / 16 llc -O2 | 26 / 17 | one insn: llc rotates the loop (tail duplication, block placement) |
| arrsum | 20 B / 68 gcc | 56 / 105 | callee-saved spills; `lsl`+indexed load for a post-increment; `add #1` |
| arrmax | 26 B / 97 gcc | 64 / 156 | as arrsum |
| countgt | 42 B / 123 gcc -O2 | 78 / 186 | as arrsum |
| strlen | 18 B / 44 gcc -O2 | 26 / 73 | copy `d0`->`d1` kept; `cmpi.b #0` for `tst.b`; `add #1` |
| copy | 34 B / 59 | 38 / 59 | 4 B |

At or below both: absdiff, clamp.

Causes seen in the asm (sum, arrsum, strlen): the pin's M68k .td has no `ADDQ`/`SUBQ` (its header marks them
`[ ]`), so llc is no better; `tst` is not selected for a compare with 0; no loop strength reduction (gcc uses
`(%a0)+`); loop invariants take callee-saved registers whose save/restore costs more than gcc's memory operand.
