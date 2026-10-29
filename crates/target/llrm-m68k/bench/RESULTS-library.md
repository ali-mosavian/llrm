# Library routines, 2026-10-29 17:27 (pin d1106deb71cc, qemu-m68k -cpu m68000)

Ten routines written after the first set was tuned (`bench/library/*.c`; IR from `gen.py`: clang -O2 without
unrolling, as gcc -O2 does, with the lines MIR's subset does not take dropped). `cargo run -p llrm-m68k --release
--example bench library | python3 -I bench/results.py bench/RESULTS-library.md`. Each routine's result is worked out
by hand (`kernels::library_table`); the muldiv and structcopy rows call the runtime in llc and gcc, so only their size
is compared. Columns as in `RESULTS.md`.

## Summary: every row against the best reference at its level

Speed level (`ours`, `raw`) against `llc -O2` and `gcc -O2`; size level (`ours -Os`: `optsize`/`minsize` or `Options::size`) against `llc -Os` and `gcc -Os`.

| kernel | raw B | ours B | best -O2 B | ours -Os B | best -Os B | raw insns | ours insns | best -O2 insns | ours -Os insns | best -Os insns | status |
|---|---|---|---|---|---|---|---|---|---|---|---|
| memcpy | 44 | 44 | 40 | 44 | 40 | 76 | 64 | 51 | 64 | 102 | -O2 +4 B, -O2 +13 insns, -Os +4 B |
| memset | 48 | 46 | 52 | 46 | 44 | 79 | 64 | 64 | 64 | 93 | -Os +2 B |
| strcmp | 50 | 48 | 82 | 48 | 48 | 89 | 79 | 102 | 79 | 104 | at or below |
| hex | 102 | 96 | 78 | 96 | 72 | 73 | 66 | 54 | 66 | 57 | -O2 +18 B, -O2 +12 insns, -Os +24 B, -Os +9 insns |
| crc16 | 92 | 94 | 86 | 94 | 76 | 708 | 636 | 629 | 636 | 630 | -O2 +8 B, -O2 +7 insns, -Os +18 B, -Os +6 insns |
| bsearch | 72 | 72 | 76 | 72 | 66 | 63 | 63 | 61 | 63 | 64 | -O2 +2 insns, -Os +6 B |
| partition | 120 | 110 | 92 | 110 | 94 | 192 | 151 | 124 | 151 | 199 | -O2 +18 B, -O2 +27 insns, -Os +16 B |
| muldiv | 80 | 80 | 64 | 80 | 64 | - | - | - | - | - | -O2 +16 B, -Os +16 B |
| structcopy | 34 | 34 | 38 | 34 | 42 | - | - | - | - | - | at or below |
| fnptr | 52 | 52 | 46 | 52 | 46 | 16 | 16 | 16 | 16 | 16 | -O2 +6 B, -Os +6 B |

Rows still above a reference: memcpy, memset, hex, crc16, bsearch, partition, muldiv, fnptr.

## Full table

| kernel | raw B | ours B | ours -Os B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | raw insns | ours insns | ours -Os insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| memcpy | 44 | 44 | 44 | 106 | 106 | 40 | 40 | 76 | 64 | 64 | 197 | 197 | 102 | 51 | 33 all |
| memset | 48 | 46 | 46 | 90 | 90 | 44 | 52 | 79 | 64 | 64 | 147 | 147 | 93 | 64 | 14 all |
| strcmp | 50 | 48 | 48 | 114 | 114 | 48 | 82 | 89 | 79 | 79 | 196 | 196 | 104 | 102 | 32 all |
| hex | 102 | 96 | 96 | 142 | 140 | 72 | 78 | 73 | 66 | 66 | 94 | 97 | 57 | 54 | 204 all |
| crc16 | 92 | 94 | 94 | 158 | 154 | 76 | 86 | 708 | 636 | 636 | 744 | 704 | 630 | 629 | 152 all |
| bsearch | 72 | 72 | 72 | 106 | 102 | 66 | 76 | 63 | 63 | 63 | 64 | 64 | 68 | 61 | 10 all |
| partition | 120 | 110 | 110 | 150 | 146 | 94 | 92 | 192 | 151 | 151 | 202 | 199 | 219 | 124 | 19 all |
| muldiv | 80 | 80 | 80 | 82 | 82 | 64 | 64 | - | - | - | - | - | - | - | 236 all |
| structcopy | 34 | 34 | 34 | 76 | 66 | 42 | 38 | - | - | - | - | - | - | - | 40 all |
| fnptr | 52 | 52 | 52 | 56 | 56 | 46 | 46 | 16 | 16 | 16 | 16 | 16 | 16 | 16 | 36 all |

## Above the best reference (bytes / insns)

| routine | gap | cause |
|---|---|---|
| memcpy, memset | +4 / +2 B (-Os) | both pointers are now `(An)+` (LSR `AMK_PostIndexed`, m68k overlay). What is left is the counter: `addq #1; subq #1; cmpi #1; bhi` where gcc counts to zero |
| hex | +24 B, +9 insns | the `or disjoint` of `'0' + d` is a `lea` through an address register (the pin has `ADD32di` for `add` only); kept for a constant over 8, where `add.l #c` is no better (8 B and 16 clocks either way; `addq` for 1 to 8 is 2 B, `lea_on_data`). The bench input (`0xBEEF`) never takes the digit arm. What is left: gcc's `add.b #48` (4 B; the high bits are known zero and the sum is under 256), and the loop test `cmpi.l #16` on the old value where gcc tests the shifted value's codes |
| crc16 | +18 B, +6 insns | `and.l #32768` + `tst.l` of one bit where `btst #15` is 4 bytes; the 8-trip inner loop counter |
| partition | +16 B, +27 insns | pointer arithmetic through data registers, repeated `lsl.l #2` of one index; two LSR index registers |
| bsearch | +6 B | the compare of a loaded value with the key through a register |
| fnptr | +6 B | the pin's reserved call frame stores arguments at `(sp)`; gcc pushes them |
| muldiv | +16 B | three calls: `__mulsi3`, `__divsi3`, and `__mulsi3` again for the remainder (llc 82, gcc 64); argument stores and the frame, not looked at yet. Its result is checked against `rt.ll`, which this compiler builds |

Fixed since this set was first run (the 32-bit multiply, divide and remainder call the runtime through a libcall table; a flag reader past a select; a compare read again after a call) (each with a run-checked test in `tests/results.rs`): two selects on one compare
(bsearch returned -1), a join phi read only by other phis (partition returned its start value), `EXTRACT_SUBREG`,
a narrowed load and a forwarded store on the wrong end of a big-endian value, `MOVI8di`/`MOVX`, static allocas (no
function with a local array compiled), and `a[i]` of a stack array, which the pin itself stores to `a[0]`.
