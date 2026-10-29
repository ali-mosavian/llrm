# Results, 2026-10-29 12:26 (pin d1106deb71cc, qemu-m68k -cpu m68000)

gcc is m68k-linux-gnu-gcc 15 (`-mcpu=68000`), the Debian cross toolchain unpacked without root. Cycles are not
modelled: "insns" are executed instructions, which price a 68000 `move` and a `muls` the same. "ours" has the
overlay on (`addq`, `subq`, `tst`, push/pop frames; `compile::Options`), a deviation from the pin's output.

| kernel | ours B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | ours insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |
|---|---|---|---|---|---|---|---|---|---|---|---|
| sum | 18 | 46 | 46 | 16 | 18 | 44 | 48 | 48 | 43 | 44 | 45 all |
| popcount | 28 | 52 | 52 | 22 | 22 | 101 | 103 | 103 | 85 | 85 | 8 all |
| max | 16 | 16 | 16 | 16 | 16 | 6 | 6 | 6 | 5 | 5 | 42 all |
| absdiff | 22 | 22 | 22 | 24 | 24 | 7 | 7 | 7 | 8 | 7 | 42 all |
| clamp | 26 | 26 | 26 | 26 | 26 | 9 | 9 | 9 | 9 | 9 | 42 all |
| fib | 32 | 66 | 66 | 22 | 28 | 77 | 139 | 139 | 64 | 67 | 89 all |
| gcd | 26 | 32 | 26 | 26 | 28 | 17 | 16 | 17 | 17 | 16 | 42 all |
| arrsum | 32 | 64 | 64 | 20 | 22 | 103 | 153 | 153 | 68 | 69 | 80 all |
| arrmax | 40 | 92 | 88 | 26 | 32 | 154 | 248 | 235 | 97 | 100 | 9 all |
| countgt | 58 | 86 | 86 | 42 | 42 | 184 | 234 | 234 | 168 | 123 | 9 all |
| strlen | 22 | 36 | 36 | 18 | 20 | 73 | 89 | 89 | 57 | 44 | 13 all |
| copy | 34 | 42 | 42 | 34 | 34 | 59 | 59 | 59 | 59 | 59 | 33 all |

## Above llc -Os or gcc -Os (bytes / insns, best reference)

| kernel | best reference | ours | gap |
|---|---|---|---|
| sum | 16 B gcc / 43 gcc | 18 / 44 | gcc folds the first trip into the loop entry (`moveq`s, then the body) |
| popcount | 22 B / 85 gcc | 28 / 101 | `lsr` already sets Z, so gcc drops the `tst`; we keep a `move`+`and` pair gcc fuses |
| max | 16 B / 5 gcc | 16 / 6 | one insn |
| fib | 22 B / 64 gcc | 32 / 77 | three phi copies the allocator keeps |
| gcd | 26 B / 16 llc | 26 / 17 | llc rotates the loop (tail duplication, block placement) |
| arrsum | 20 B / 68 gcc | 32 / 103 | `(%a0)+` post-increment; we shift and index (no loop strength reduction) |
| arrmax | 26 B / 97 gcc | 40 / 154 | as arrsum |
| countgt | 42 B / 123 gcc -O2 | 58 / 184 | as arrsum |
| strlen | 18 B / 44 gcc -O2 | 22 / 73 | post-increment; gcc -O2 unrolls the test |

At or below both: absdiff, clamp, copy. Smaller than llc on every row except max, absdiff, clamp, gcd (equal).

Done since the last table: MachineSink, branch folding, spilling with rematerialization, relaxation, the overlay,
caller-saved registers first. Next: reusing the flags an instruction already set, post-increment addressing.
