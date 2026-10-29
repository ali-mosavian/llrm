# Results, 2026-10-29 13:24 (pin d1106deb71cc, qemu-m68k -cpu m68000)

gcc is m68k-linux-gnu-gcc 15 (`-mcpu=68000`), the Debian cross toolchain unpacked without root. Cycles are not
modelled: "insns" are executed instructions, which price a 68000 `move` and a `muls` the same. "ours" has the
overlay on (`addq`, `subq`, `tst`, push/pop frames, `(An)+`; `compile::Options`) and the LSR mid-end, a deviation
from the pin's output.

**raw** is our output on the IR as written; **ours** adds the loop strength reduction `llc` also runs (the llc and gcc
columns include theirs). Compare ours with llc, raw with nothing. The mid-end is LSR only, priced by the m68k `Tti`:
the full -O2 pipeline closed-forms loops into i33 arithmetic the legalizer cannot lower yet.

| kernel | raw B | ours B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | raw insns | ours insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| sum | 18 | 18 | 46 | 46 | 16 | 18 | 44 | 44 | 48 | 48 | 43 | 44 | 45 all |
| popcount | 26 | 26 | 52 | 52 | 22 | 22 | 85 | 85 | 103 | 103 | 85 | 85 | 8 all |
| max | 16 | 16 | 16 | 16 | 16 | 16 | 6 | 6 | 6 | 6 | 5 | 5 | 42 all |
| absdiff | 22 | 22 | 22 | 22 | 24 | 24 | 7 | 7 | 7 | 7 | 8 | 7 | 42 all |
| clamp | 26 | 26 | 26 | 26 | 26 | 26 | 9 | 9 | 9 | 9 | 9 | 9 | 42 all |
| fib | 28 | 28 | 66 | 66 | 22 | 28 | 67 | 67 | 139 | 139 | 64 | 67 | 89 all |
| gcd | 26 | 26 | 32 | 26 | 26 | 28 | 17 | 17 | 16 | 17 | 17 | 16 | 42 all |
| arrsum | 32 | 22 | 64 | 64 | 20 | 22 | 103 | 69 | 153 | 153 | 68 | 69 | 80 all |
| arrmax | 26 B / 97 gcc | 34 / 115 | the `(a0)` load and `addq #4` sit in different blocks (no `(a0)+`); the loaded value and `n` need callee-saved `d2` pushed (gcc compares with `n` in memory) |
| countgt | 52 | 46 | 86 | 86 | 42 | 42 | 168 | 136 | 234 | 234 | 168 | 123 | 9 all |
| strlen | 20 | 22 | 36 | 36 | 18 | 20 | 59 | 60 | 89 | 89 | 57 | 44 | 13 all |
| copy | 34 | 34 | 42 | 42 | 34 | 34 | 59 | 59 | 59 | 59 | 59 | 59 | 33 all |

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
