# Softfloat C through gcc vs hand Z80 asm, 2026-11-03

**STALE 2026-11-04: measured from runtime/softfloat/src (the monolithic sf32.c), which a286a52c00 moved into runtime/compiler-rt (one file per operation); re-measure on compiler-rt due 2026-11-06.**

TL;DR: gcc is 6.9-31x slower than `src/z80/*.s` per op and 7.2x larger (18004 B vs 2491 B, smallest cell); every gcc result is bit-exact. Cells: `softfloat-z80.tsv`.

Method: `runtime/softfloat/src/sf32*.c` + `int32.c` (`sf32` generic, `sf32_6502` 6502-shaped), `-DSF_INTABI`, -O2/-Os, spike gcc Z80 backend (arguments on the stack,
result in e d l h; a driver pushes them like gcc), against `sf32_arith.s`, `sf32_cmp.s`, `int32.s` through `tests/z80/drv.s`.
Same records, ucsim_z80; clk = (run - bare-`ret` stub run) / records; the stub run passes every record (positive control). gcc: 0 fails in all 32 cells.
Records with a NaN input are left out of add/sub/mul/div/ftoi/ftou on both sides: `sf32.c` propagates the payload, the hand asm returns the canonical NaN
(a harness difference, not a bug: the failing add record passes when the payload is expected).

## Clocks per call (best gcc cell over variant and level)

| op | hand | gcc | gcc/hand | gcc cell |
|---|---|---|---|---|
| add | 1537 | 10846 | 7.1x | -O2 sf32 |
| sub | 1576 | 10819 | 6.9x | -O2 sf32 |
| mul | 2774 | 87251 | 31.5x | -O2 sf32_6502 |
| div | 3585 | 43920 | 12.3x | -O2 sf32 |
| itof | 658 | 8813 | 13.4x | -O2 sf32 |
| utof | 531 | 9102 | 17.1x | -O2 sf32 |
| ftoi | 274 | 3321 | 12.1x | -Os sf32 |
| ftou | 245 | 3067 | 12.5x | -O2 sf32 |

## Library bytes (_CODE, helpers included)

| | hand | gcc -Os | gcc -O2 |
|---|---|---|---|
| sf32 generic | 2491 | 18162 | 33374 |
| sf32_6502 variant | - | 18004 | 36731 |

## Limits

- The Z80 backend has no native 32-bit add/sub/logic/compare patterns: gcc expands them into byte operations, and 32-bit shifts are library calls
  (`__ashlsi3`, `__lshrsi3`; hand-written asm). `__mulsi3` in these cells is the gcc-compiled one from `int32.c`. The gcc column is an upper bound.
- Every call pushes and pops its 4 or 8 argument bytes on the stack; the hand asm passes in registers. This is part of the gap on the small ops (ftoi, ftou).
- Not measured: llrm-cc on the same C; -O1.
