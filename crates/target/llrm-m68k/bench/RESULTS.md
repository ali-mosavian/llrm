# Results, 2026-10-29 12:51 (pin d1106deb71cc, qemu-m68k -cpu m68000)

gcc is m68k-linux-gnu-gcc 15 (`-mcpu=68000`), the Debian cross toolchain unpacked without root. Cycles are not
modelled: "insns" are executed instructions, which price a 68000 `move` and a `muls` the same. "ours" has the
overlay on (`addq`, `subq`, `tst`, push/pop frames, `(An)+`; `compile::Options`) and the LSR mid-end, a deviation
from the pin's output.

| kernel | ours B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | ours insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |
|---|---|---|---|---|---|---|---|---|---|---|---|
| sum | 18 | 46 | 46 | 16 | 18 | 44 | 48 | 48 | 43 | 44 | 45 all |
| popcount | 26 | 52 | 52 | 22 | 22 | 85 | 103 | 103 | 85 | 85 | 8 all |
| max | 16 | 16 | 16 | 16 | 16 | 6 | 6 | 6 | 5 | 5 | 42 all |
| absdiff | 22 | 22 | 22 | 24 | 24 | 7 | 7 | 7 | 8 | 7 | 42 all |
| clamp | 26 | 26 | 26 | 26 | 26 | 9 | 9 | 9 | 9 | 9 | 42 all |
| fib | 32 | 66 | 66 | 22 | 28 | 77 | 139 | 139 | 64 | 67 | 89 all |
| gcd | 26 | 32 | 26 | 26 | 28 | 17 | 16 | 17 | 17 | 16 | 42 all |
| arrsum | 22 | 64 | 64 | 20 | 22 | 69 | 153 | 153 | 68 | 69 | 80 all |
| arrmax | 36 | 92 | 88 | 26 | 32 | 139 | 248 | 235 | 97 | 100 | 9 all |
| countgt | 52 | 86 | 86 | 42 | 42 | 152 | 234 | 234 | 168 | 123 | 9 all |
| strlen | 24 | 36 | 36 | 18 | 20 | 74 | 89 | 89 | 57 | 44 | 13 all |
| copy | 34 | 42 | 42 | 34 | 34 | 59 | 59 | 59 | 59 | 59 | 33 all |

## Above llc -Os or gcc -Os (bytes / insns, best reference)

| kernel | best reference | ours | gap |
|---|---|---|---|
| sum | 16 B gcc / 43 gcc | 18 / 44 | gcc folds the first trip into the loop entry |
| popcount | 22 B / 85 gcc | 26 / 85 | a `move`+`and` pair gcc fuses |
| max | 16 B / 5 gcc | 16 / 6 | one insn |
| fib | 22 B / 64 gcc | 32 / 77 | three phi copies the allocator keeps |
| gcd | 26 B / 16 llc | 26 / 17 | llc rotates the loop (tail duplication, block placement) |
| arrsum | 20 B / 68 gcc | 22 / 69 | gcc's loop test is `cmp` against the end pointer; ours keeps a counter |
| arrmax | 26 B / 97 gcc | 36 / 139 | counter kept beside the pointer; max not folded |
| countgt | 42 B / 123 gcc -O2 | 52 / 152 | as arrmax |
| strlen | 18 B / 44 gcc -O2 | 24 / 74 | a phi copy pair in the loop; gcc -O2 unrolls the test |

At or below both: absdiff, clamp, copy. Smaller than llc on every row except max, absdiff, clamp, gcd (equal).

Done since the last table: LSR with a proper `Machine`/`Tti`, flags reuse, `preserve_flags`, pointer arithmetic in `An`,
`(An)+` folding, `add #-n` as `subq`. Next: phi-copy coalescing (fib, strlen), then the counter against the end pointer.
