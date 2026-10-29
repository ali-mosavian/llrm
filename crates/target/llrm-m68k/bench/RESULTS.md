# Results, 2026-10-29 13:47 (pin d1106deb71cc, qemu-m68k -cpu m68000)

gcc is m68k-linux-gnu-gcc 15 (`-mcpu=68000`), the Debian cross toolchain unpacked without root. Cycles are not
modelled: "insns" are executed instructions, which price a 68000 `move` and a `muls` the same. "ours" has the
overlay on (`addq`, `subq`, `tst`, push/pop frames, `(An)+`; `compile::Options`) and the LSR mid-end, a deviation
from the pin's output.

**raw** is our output on the IR as written; **ours** adds the loop strength reduction `llc` also runs (the llc and gcc
columns include theirs). Compare ours with llc, raw with nothing. The mid-end is LSR only, priced by the m68k `Tti`:
the full -O2 pipeline closed-forms loops into i33 arithmetic the legalizer cannot lower yet.

## Summary: every row against the best reference at its level

Speed level (`ours`, `raw`) against `llc -O2` and `gcc -O2`; size level (`ours -Os`: `optsize`/`minsize` or `Options::size`) against `llc -Os` and `gcc -Os`.

| kernel | raw B | ours B | best -O2 B | ours -Os B | best -Os B | raw insns | ours insns | best -O2 insns | ours -Os insns | best -Os insns | status |
|---|---|---|---|---|---|---|---|---|---|---|---|
| sum | 18 | 18 | 18 | 16 | 16 | 44 | 44 | 44 | 43 | 43 | at or below |
| popcount | 22 | 22 | 22 | 22 | 22 | 85 | 85 | 85 | 85 | 85 | at or below |
| max | 16 | 16 | 16 | 16 | 16 | 6 | 6 | 5 | 6 | 5 | -O2 +1 insns, -Os +1 insns |
| absdiff | 22 | 22 | 22 | 22 | 22 | 7 | 7 | 7 | 7 | 7 | at or below |
| clamp | 26 | 26 | 26 | 26 | 26 | 9 | 9 | 9 | 9 | 9 | at or below |
| fib | 28 | 28 | 28 | 26 | 22 | 67 | 67 | 67 | 66 | 64 | -Os +4 B, -Os +2 insns |
| gcd | 26 | 26 | 28 | 26 | 26 | 17 | 17 | 16 | 17 | 17 | -O2 +1 insns |
| arrsum | 32 | 22 | 22 | 20 | 20 | 103 | 69 | 69 | 68 | 68 | at or below |
| arrmax | 38 | 34 | 32 | 32 | 26 | 130 | 115 | 100 | 114 | 97 | -O2 +2 B, -O2 +15 insns, -Os +6 B, -Os +17 insns |
| countgt | 50 | 44 | 42 | 38 | 42 | 152 | 120 | 123 | 134 | 168 | -O2 +2 B |
| strlen | 18 | 18 | 20 | 18 | 18 | 59 | 59 | 44 | 59 | 57 | -O2 +15 insns, -Os +2 insns |
| copy | 34 | 34 | 34 | 34 | 34 | 59 | 59 | 59 | 59 | 59 | at or below |

Rows still above a reference: max, fib, gcd, arrmax, countgt, strlen.

## Full table

| kernel | raw B | ours B | ours -Os B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | raw insns | ours insns | ours -Os insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| sum | 18 | 18 | 16 | 46 | 46 | 16 | 18 | 44 | 44 | 43 | 48 | 48 | 43 | 44 | 45 all |
| popcount | 22 | 22 | 22 | 52 | 52 | 22 | 22 | 85 | 85 | 85 | 103 | 103 | 85 | 85 | 8 all |
| max | 16 | 16 | 16 | 16 | 16 | 16 | 16 | 6 | 6 | 6 | 6 | 6 | 5 | 5 | 42 all |
| absdiff | 22 | 22 | 22 | 22 | 22 | 24 | 24 | 7 | 7 | 7 | 7 | 7 | 8 | 7 | 42 all |
| clamp | 26 | 26 | 26 | 26 | 26 | 26 | 26 | 9 | 9 | 9 | 9 | 9 | 9 | 9 | 42 all |
| fib | 28 | 28 | 26 | 66 | 66 | 22 | 28 | 67 | 67 | 66 | 139 | 139 | 64 | 67 | 89 all |
| gcd | 26 | 26 | 26 | 32 | 26 | 26 | 28 | 17 | 17 | 17 | 16 | 17 | 17 | 16 | 42 all |
| arrsum | 32 | 22 | 20 | 64 | 64 | 20 | 22 | 103 | 69 | 68 | 153 | 153 | 68 | 69 | 80 all |
| arrmax | 38 | 34 | 32 | 92 | 88 | 26 | 32 | 130 | 115 | 114 | 248 | 235 | 97 | 100 | 9 all |
| countgt | 50 | 44 | 38 | 86 | 86 | 42 | 42 | 152 | 120 | 134 | 234 | 234 | 168 | 123 | 9 all |
| strlen | 18 | 18 | 18 | 36 | 36 | 18 | 20 | 59 | 59 | 59 | 89 | 89 | 57 | 44 | 13 all |
| copy | 34 | 34 | 34 | 42 | 42 | 34 | 34 | 59 | 59 | 59 | 59 | 59 | 59 | 59 | 33 all |

## Above llc -Os or gcc -Os (bytes / insns, best reference)

| kernel | best reference | ours | gap |
|---|---|---|---|
| sum | 16 B gcc / 43 gcc | 18 / 44 | gcc folds the first trip into the loop entry |
| popcount | 22 B / 85 gcc | 26 / 85 | a `move`+`and` pair gcc fuses |
| max | 16 B / 5 gcc | 16 / 6 | one insn; 6 in every earlier table too, not a regression |
| fib | 22 B / 64 gcc | 32 / 77 | three phi copies the allocator keeps |
| gcd | 26 B / 16 llc | 26 / 17 | llc rotates the loop (tail duplication, block placement) |
| arrsum | 20 B / 68 gcc | 22 / 69 | gcc's loop test is `cmp` against the end pointer; ours keeps a counter |
| arrmax | 26 B / 97 gcc | 36 / 139 | counter kept beside the pointer; max not folded |
| countgt | 42 B / 123 gcc -O2 | 52 / 152 | as arrmax |
| strlen | 18 B / 44 gcc -O2 | 24 / 74 | LSR moved it from 22 raw to 24: a phi copy pair in the loop; gcc -O2 unrolls the test |

At or below both: absdiff, clamp, copy. Smaller than llc on every row except max, absdiff, clamp, gcd (equal).

Done since the last table: LSR with a proper `Machine`/`Tti`, flags reuse, `preserve_flags`, pointer arithmetic in `An`,
`(An)+` folding, `add #-n` as `subq`. `MachineCopyPropagation`'s spurious-copy case (strlen 24 to 22). The pin's commute step in two-address, `add` with an address-register source commutable, phi copies before the source's last read (fib 32 to 28, 77 to 67 insns; llc also emits three moves there). Next: the counter against the end pointer.

Limit: the commute and phi-copy placement save a move on a loop that carries a sum (fib, 3 moves to 2). A rotation
of three values with a sum (tribonacci) stays at 4 moves, as in the pin; `tests/overlay.rs` pins fib's shape only.
Between the 32 B and 28 B fib tables only fib changed (32 to 28); no other row grew.
