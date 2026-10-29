# Results, 2026-10-29 17:56 (pin d1106deb71cc, qemu-m68k -cpu m68000)

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
| fib | 28 | 28 | 28 | 22 | 22 | 67 | 67 | 67 | 64 | 64 | at or below |
| gcd | 26 | 26 | 28 | 26 | 26 | 17 | 17 | 16 | 17 | 17 | -O2 +1 insns |
| arrsum | 32 | 22 | 22 | 20 | 20 | 103 | 69 | 69 | 68 | 68 | at or below |
| arrmax | 38 | 32 | 32 | 26 | 26 | 130 | 100 | 100 | 97 | 97 | at or below |
| countgt | 50 | 42 | 42 | 38 | 42 | 152 | 136 | 123 | 134 | 168 | -O2 +13 insns |
| strlen | 18 | 14 | 20 | 16 | 18 | 59 | 45 | 44 | 45 | 57 | -O2 +1 insns |
| copy | 34 | 36 | 34 | 34 | 34 | 59 | 62 | 59 | 59 | 59 | -O2 +2 B, -O2 +3 insns |

Rows still above a reference: max, gcd, countgt, strlen, copy.

## Full table

| kernel | raw B | ours B | ours -Os B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | raw insns | ours insns | ours -Os insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| sum | 18 | 18 | 16 | 46 | 46 | 16 | 18 | 44 | 44 | 43 | 48 | 48 | 43 | 44 | 45 all |
| popcount | 22 | 22 | 22 | 52 | 52 | 22 | 22 | 85 | 85 | 85 | 103 | 103 | 85 | 85 | 8 all |
| max | 16 | 16 | 16 | 16 | 16 | 16 | 16 | 6 | 6 | 6 | 6 | 6 | 5 | 5 | 42 all |
| absdiff | 22 | 22 | 22 | 22 | 22 | 24 | 24 | 7 | 7 | 7 | 7 | 7 | 8 | 7 | 42 all |
| clamp | 26 | 26 | 26 | 26 | 26 | 26 | 26 | 9 | 9 | 9 | 9 | 9 | 9 | 9 | 42 all |
| fib | 28 | 28 | 22 | 66 | 66 | 22 | 28 | 67 | 67 | 64 | 139 | 139 | 64 | 67 | 89 all |
| gcd | 26 | 26 | 26 | 32 | 26 | 26 | 28 | 17 | 17 | 17 | 16 | 17 | 17 | 16 | 42 all |
| arrsum | 32 | 22 | 20 | 64 | 64 | 20 | 22 | 103 | 69 | 68 | 153 | 153 | 68 | 69 | 80 all |
| arrmax | 38 | 32 | 26 | 92 | 88 | 26 | 32 | 130 | 100 | 97 | 248 | 235 | 97 | 100 | 9 all |
| countgt | 50 | 42 | 38 | 86 | 86 | 42 | 42 | 152 | 136 | 134 | 234 | 234 | 168 | 123 | 9 all |
| strlen | 18 | 14 | 16 | 36 | 36 | 18 | 20 | 59 | 45 | 45 | 89 | 89 | 57 | 44 | 13 all |
| copy | 34 | 36 | 34 | 42 | 42 | 34 | 34 | 59 | 62 | 59 | 59 | 59 | 59 | 59 | 33 all |

## Above the best reference (bytes / insns)

| kernel | level | reference | ours | gap |
|---|---|---|---|---|
| max | both | 5 insns gcc | 6 (= llc) | not a gap: gcc's 5 holds only for the bench's input (a <= b); for a > b gcc runs 6 and ours 5 |
| gcd | -O2 | 16 insns llc | 17 | llc rotates the loop (loop rotation, neutral-LIR task; tail duplication and block placement held) |
| countgt | -O2 | 123 insns gcc | 136 | +1 insn per trip by choice: `moveq #0; slt; neg.b` for `slt; and.l #1`, 8 clocks and 2 B less (68000: `and.l #imm,Dn` 16, `moveq` 4, `neg.b Dn` 4); bytes now = gcc (42) |
| strlen | -O2 | 44 insns gcc | 45 | gcc peels the first test: loop peeling, full -O2 pipeline after the i33 legalizer gap |
| copy | -O2 | 34 B, 59 insns gcc | 36 B, 62 | by choice: LSR (post-indexed, as the pin does for AArch64) makes two `(An)+` pointers, `d` is live after the loop so one is a copy of it. A trip is 12 clocks less (PRM Table 8-1, byte effective addresses: `(An)+` 4+4 against `(d8,An,Xn)` 10+10); setup is about 38 clocks more (`move.l`, save and restore of `%a2`), so it wins from 4 trips. Off at the size level, where copy stays 34 B |

At or below both: sum, popcount, absdiff, clamp, fib, arrsum, arrmax; copy at -Os; strlen in bytes and at -Os.

## Open: the `neg.b` rewrite

`boolean_by_negation` (countgt) is register-choice dependent: it needs `cmp; scc; and.l #1` adjacent and a compare that does not read the register `scc` writes, so it misses when the allocator picks one register for both (`slt %d0` after `cmp.l (8,%sp), %d0`) and when a move sits between. It is never wrong, only sometimes missed (run tests in `tests/results.rs`). The fix is MIR-level, before the allocator: `SETcc` into a fresh byte register of a pre-zeroed long, then `neg.b`, so no register choice is involved.
