# Library routines, 2026-10-29 14:51 (pin d1106deb71cc, qemu-m68k -cpu m68000)

Ten routines written after the first set was tuned (`bench/library/*.c`; IR from `gen.py`: clang -O2 without
unrolling, as gcc -O2 does, with the lines MIR's subset does not take dropped). `cargo run -p llrm-m68k --release
--example bench library | python3 -I bench/results.py bench/RESULTS-library.md`. Each routine's result is worked out
by hand (`kernels::library_table`); the muldiv and structcopy rows call the runtime in llc and gcc, so only their size
is compared. Columns as in `RESULTS.md`.

## Summary: every row against the best reference at its level

Speed level (`ours`, `raw`) against `llc -O2` and `gcc -O2`; size level (`ours -Os`: `optsize`/`minsize` or `Options::size`) against `llc -Os` and `gcc -Os`.

| kernel | raw B | ours B | best -O2 B | ours -Os B | best -Os B | raw insns | ours insns | best -O2 insns | ours -Os insns | best -Os insns | status |
|---|---|---|---|---|---|---|---|---|---|---|---|
| memcpy | 64 | 46 | 40 | 46 | 40 | 180 | 75 | 51 | 75 | 102 | -O2 +6 B, -O2 +24 insns, -Os +6 B |
| memset | 48 | 48 | 52 | 48 | 44 | 79 | 80 | 64 | 80 | 93 | -O2 +16 insns, -Os +4 B |
| strcmp | 62 | 48 | 82 | 48 | 48 | 125 | 79 | 102 | 79 | 104 | at or below |
| hex | 102 | 96 | 78 | 96 | 72 | 81 | 66 | 54 | 66 | 57 | -O2 +18 B, -O2 +12 insns, -Os +24 B, -Os +9 insns |
| crc16 | 92 | 94 | 86 | 94 | 76 | 708 | 636 | 629 | 636 | 630 | -O2 +8 B, -O2 +7 insns, -Os +18 B, -Os +6 insns |
| bsearch | 72 | 72 | 76 | 72 | 66 | 63 | 63 | 61 | 63 | 64 | -O2 +2 insns, -Os +6 B |
| partition | 120 | 110 | 92 | 110 | 94 | 192 | 151 | 124 | 151 | 199 | -O2 +18 B, -O2 +27 insns, -Os +16 B |
| muldiv | - | - | 64 | - | 64 | - | - | - | - | - | fails: fails: Legalize("Unsupported { op: Mul, ty: Simple(I32), what: \"expand node\" } |
| structcopy | 54 | 54 | 38 | 54 | 42 | - | - | - | - | - | -O2 +16 B, -Os +12 B |
| fnptr | 52 | 52 | 46 | 52 | 46 | 16 | 16 | 16 | 16 | 16 | -O2 +6 B, -Os +6 B |

Rows still above a reference: memcpy, memset, hex, crc16, bsearch, partition, muldiv, structcopy, fnptr.

## Full table

| kernel | raw B | ours B | ours -Os B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | raw insns | ours insns | ours -Os insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| memcpy | 64 | 46 | 46 | 106 | 106 | 40 | 40 | 180 | 75 | 75 | 197 | 197 | 102 | 51 | 33 all |
| memset | 48 | 48 | 48 | 90 | 90 | 44 | 52 | 79 | 80 | 80 | 147 | 147 | 93 | 64 | 14 all |
| strcmp | 62 | 48 | 48 | 114 | 114 | 48 | 82 | 125 | 79 | 79 | 196 | 196 | 104 | 102 | 32 all |
| hex | 102 | 96 | 96 | 142 | 140 | 72 | 78 | 81 | 66 | 66 | 94 | 97 | 57 | 54 | 204 all |
| crc16 | 92 | 94 | 94 | 158 | 154 | 76 | 86 | 708 | 636 | 636 | 744 | 704 | 630 | 629 | 152 all |
| bsearch | 72 | 72 | 72 | 106 | 102 | 66 | 76 | 63 | 63 | 63 | 64 | 64 | 68 | 61 | 10 all |
| partition | 120 | 110 | 110 | 150 | 146 | 94 | 92 | 192 | 151 | 151 | 202 | 199 | 219 | 124 | 19 all |
| muldiv | - | - | - | 82 | 82 | 64 | 64 | - | - | - | - | - | - | - | fails: Legalize("Unsupported { op: Mul, ty: Simple(I32), what: \"expand node\" }") / fails: Legalize("Unsupported { op: Mul, ty: Simple(I32), what: \"expand node\" }") / fails: Legalize("Unsupported { op: Mul, ty: Simple(I32), what: \"expand node\" }") / ok / ok / ok / ok |
| structcopy | 54 | 54 | 54 | 76 | 66 | 42 | 38 | - | - | - | - | - | - | - | 40 all |
| fnptr | 52 | 52 | 52 | 56 | 56 | 46 | 46 | 16 | 16 | 16 | 16 | 16 | 16 | 16 | 36 all |

## Above the best reference (bytes / insns)

| routine | gap | cause |
|---|---|---|
| memcpy, memset | +6 / +4 B, +24 / +16 insns | LSR shares one index register between the two pointers; gcc runs two `(An)+` pointers. The pin's LSR has `AMK_PostIndexed` for this; the shared LSR does not port it (`lsr_solve.rs`), and the m68k `Tti` has no indexed-access hooks. Proposed: port it |
| hex | +24 B, +9 insns | an `or disjoint` selected as `lea` through an address register (3 instructions where gcc has `add.b #48`); the loop test `cmpi.l #16` on the old value where gcc tests the shifted value's codes |
| crc16 | +18 B, +6 insns | `and.l #32768` + `tst.l` of one bit where `btst #15` is 4 bytes; the 8-trip inner loop counter |
| partition | +16 B, +27 insns | pointer arithmetic through data registers, repeated `lsl.l #2` of one index; two LSR index registers |
| structcopy | +12 B | the pin's memcpy expansion stores with displacements (6 B a pair); gcc walks both pointers with `(An)+` (2 B a pair) |
| bsearch | +6 B | the compare of a loaded value with the key through a register |
| fnptr | +6 B | the pin's reserved call frame stores arguments at `(sp)`; gcc pushes them |
| muldiv | does not compile | no `mul`/`div` libcall conversion in the shared legalizer (held, S2) |

Fixed since this set was first run (each with a run-checked test in `tests/results.rs`): two selects on one compare
(bsearch returned -1), a join phi read only by other phis (partition returned its start value), `EXTRACT_SUBREG`,
a narrowed load and a forwarded store on the wrong end of a big-endian value, `MOVI8di`/`MOVX`, static allocas (no
function with a local array compiled), and `a[i]` of a stack array, which the pin itself stores to `a[0]`.
