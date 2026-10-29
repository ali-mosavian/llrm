# Library routines, 2026-10-29 18:24 (pin d1106deb71cc, qemu-m68k -cpu m68000)

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
| strcmp | 50 | 48 | 82 | 48 | 48 | 90 | 80 | 102 | 80 | 104 | at or below |
| hex | 102 | 96 | 78 | 96 | 72 | 73 | 66 | 54 | 66 | 57 | -O2 +18 B, -O2 +12 insns, -Os +24 B, -Os +9 insns |
| crc16 | 90 | 92 | 86 | 92 | 76 | 699 | 628 | 629 | 628 | 630 | -O2 +6 B, -Os +16 B |
| bsearch | 70 | 70 | 76 | 70 | 66 | 60 | 60 | 61 | 60 | 64 | -Os +4 B |
| partition | 112 | 104 | 92 | 100 | 94 | 170 | 121 | 124 | 118 | 199 | -O2 +12 B, -Os +6 B |
| muldiv | 68 | 68 | 64 | 68 | 64 | - | - | - | - | - | -O2 +4 B, -Os +4 B |
| structcopy | 34 | 34 | 38 | 34 | 42 | - | - | - | - | - | at or below |
| fnptr | 40 | 40 | 46 | 40 | 46 | 16 | 16 | 16 | 16 | 16 | at or below |

Rows still above a reference: memcpy, memset, hex, crc16, bsearch, partition, muldiv.

## Full table

| kernel | raw B | ours B | ours -Os B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | raw insns | ours insns | ours -Os insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| memcpy | 44 | 44 | 44 | 106 | 106 | 40 | 40 | 76 | 64 | 64 | 197 | 197 | 102 | 51 | 33 all |
| memset | 48 | 46 | 46 | 90 | 90 | 44 | 52 | 79 | 64 | 64 | 147 | 147 | 93 | 64 | 14 all |
| strcmp | 50 | 48 | 48 | 114 | 114 | 48 | 82 | 90 | 80 | 80 | 196 | 196 | 104 | 102 | 32 all |
| hex | 102 | 96 | 96 | 142 | 140 | 72 | 78 | 73 | 66 | 66 | 94 | 97 | 57 | 54 | 204 all |
| crc16 | 90 | 92 | 92 | 158 | 154 | 76 | 86 | 699 | 628 | 628 | 744 | 704 | 630 | 629 | 152 all |
| bsearch | 70 | 70 | 70 | 106 | 102 | 66 | 76 | 60 | 60 | 60 | 64 | 64 | 68 | 61 | 10 all |
| partition | 112 | 104 | 100 | 150 | 146 | 94 | 92 | 170 | 121 | 118 | 202 | 199 | 219 | 124 | 19 all |
| muldiv | 68 | 68 | 68 | 82 | 82 | 64 | 64 | - | - | - | - | - | - | - | 236 all |
| structcopy | 34 | 34 | 34 | 76 | 66 | 42 | 38 | - | - | - | - | - | - | - | 40 all |
| fnptr | 40 | 40 | 40 | 56 | 56 | 46 | 46 | 16 | 16 | 16 | 16 | 16 | 16 | 16 | 36 all |

## Above the best reference (bytes / insns)

| routine | gap | cause |
|---|---|---|
| memcpy, memset | +4 / +2 B (-Os) | both pointers are now `(An)+` (LSR `AMK_PostIndexed`, m68k overlay). What is left is the counter: `addq #1; subq #1; cmpi #1; bhi` where gcc counts to zero |
| hex | +24 B, +9 insns | the `or disjoint` of `'0' + d` is a `lea` through an address register (the pin has `ADD32di` for `add` only); kept for a constant over 8, where `add.l #c` is no better (8 B and 16 clocks either way; `addq` for 1 to 8 is 2 B, `lea_on_data`). The bench input (`0xBEEF`) never takes the digit arm. What is left: gcc's `add.b #48` (4 B; the high bits are known zero and the sum is under 256), and the loop test `cmpi.l #16` on the old value where gcc tests the shifted value's codes. Not a missing mid-end pass: the pin's `opt -passes=aggressive-instcombine,instcombine` leaves it alone (`TruncInstCombine::getBestTruncatedType` refuses a select with a wide user, here the sum), and `llc` emits `add.l #87` the same way; gcc narrows at tree level |
| crc16 | +16 B | `and.l #32768` + `tst.l` of one bit where `btst #15` is 4 bytes. The pin does not emit it for a constant mask (`LowerAndToBTST` only for `(1 << n) & x`, `(x >> n) & 1` and masks past 32 bits) and gcc -O2 does not either; `btst #15` tried as an overlay cost crc16 +40 insns and +2 B: the shifted copy of the loop-carried value lands in a second register and both arms of the select copy it back (two-address copy on the shift rather than on the test; allocator, neutral-LIR). The 8-trip counter |
| partition | see summary | block placement rotates the loop (151 to 141 insns), the latch's step becomes the header's `(%a1)+` (141 to 126), and `&a[n-1]` is a `lea (0,%a0,%d1)` instead of an `add.l` in a callee-saved data register (126 to 121; `address.rs::base_plus_index`). What is left: the prologue's `movem` of d2-d5 and a2, repeated `lsl.l #2` of one index, two LSR index registers |
| bsearch | -Os +4 B | the key and the loaded value through registers where gcc compares `20(%sp)` directly; the repeated `cmp` after `beq` is gone (`drop_repeated_compares`, 63 to 60 insns; gcd 17 to 15 by the same) |
| fnptr | at or below | saved registers pushed first, the rest of the frame after (`frame.rs::pushes`: 52 to 40 B) |
| muldiv | +4 B | (68 B after the frame change from 80) three calls: `__mulsi3`, `__divsi3`, and `__mulsi3` again for the remainder (llc 82, gcc 64); argument stores and the frame, not looked at yet. Its result is checked against `rt.ll`, which this compiler builds |

Block placement (`MachineBlockPlacement`, `llrm-spike-mf/src/placement.rs`) moved three rows against the same build without it: partition -10 insns (-13 at -Os, +2 B at -O2, -2 B at -Os), strcmp +1 insn and crc16 +1 insn from placement alone (crc16 then -9 from the cross-block `(An)+`). Both +1 are the pin's choice: a guard over a loop is 37.5% likely (`x == 0` heuristic), so the cold return is laid out before the join and the loop exit jumps over it; `llc` lays strcmp out the same way.

Fixed since this set was first run (the 32-bit multiply, divide and remainder call the runtime through a libcall table; a flag reader past a select; a compare read again after a call) (each with a run-checked test in `tests/results.rs`): two selects on one compare
(bsearch returned -1), a join phi read only by other phis (partition returned its start value), `EXTRACT_SUBREG`,
a narrowed load and a forwarded store on the wrong end of a big-endian value, `MOVI8di`/`MOVX`, static allocas (no
function with a local array compiled), and `a[i]` of a stack array, which the pin itself stores to `a[0]`.
