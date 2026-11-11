# Softfloat C through gcc vs hand Z80 asm, 2026-11-04

TL;DR: with native 32-bit add/sub/move/compare gcc is 5.5-27.5x slower than `config/z80/*.s` per op and 4.7x larger (11748 B vs 2491 B, smallest cell); every gcc result is bit-exact. Cells: `softfloat-z80.tsv`.

Method: `runtime/compiler-rt` (`lib/fp/f32/*.c`, `lib/int/int32.c`; `sf32_6502` = `config/6502/sf32_6502.c`, one translation unit per operation), `-DSF_INTABI`, -O2/-Os,
spike gcc Z80 backend (arguments on the stack, result in e d l h; a driver pushes them like gcc), against `config/z80/{sf32_arith,sf32_cmp,int32}.s` through `tests/z80/drv.s`.
Built 2026-11-04 01:27 CET, cc1 md5 3e9f5f3199c2b23071f604505203ba7f (fork spike/z80-spadj e22a76464). Hand cells carried from 11-03 (the .s files are unchanged since the move).
Same records, ucsim_z80; clk = (run - bare-`ret` stub run) / records; gcc: 0 fails in all 32 cells.
Records with a NaN input are left out of add/sub/mul/div/ftoi/ftou on both sides: the C propagates the payload, the hand asm returns the canonical NaN
(a harness difference, not a bug: the failing add record passes when the payload is expected).

## Clocks per call (best gcc cell over variant and level)

| op | hand | gcc | gcc/hand | gcc cell |
|---|---|---|---|---|
| add | 1537 | 8604 | 5.6x | -Os sf32_6502 |
| sub | 1576 | 8599 | 5.5x | -Os sf32_6502 |
| mul | 2774 | 76281 | 27.5x | -O2 sf32_6502 |
| div | 3585 | 31613 | 8.8x | -O2 sf32 |
| itof | 658 | 8774 | 13.3x | -Os sf32_6502 |
| utof | 531 | 6391 | 12.0x | -Os sf32_6502 |
| ftoi | 274 | 2846 | 10.4x | -O2 sf32 |
| ftou | 245 | 2257 | 9.2x | -O2 sf32 |

## Library bytes (_CODE, helpers included)

| | hand | gcc -Os | gcc -O2 |
|---|---|---|---|
| sf32 generic | 2491 | 11752 | 17752 |
| sf32_6502 variant | - | 11748 | 19522 |

Against the previous cc1 (38cd2b7d): clocks 0.93-0.99x (geomean 0.95), library bytes -7..-11%, no cell worse; the 30 kernels are identical (90/90).

## Limits

- 32-bit logic and shifts are still byte operations or library calls (native: add, sub, move, compare)
  (`__ashlsi3`, `__lshrsi3`; hand-written asm). `__mulsi3` in these cells is the gcc-compiled one from `int32.c`. The gcc column is an upper bound.
- Every call pushes and pops its 4 or 8 argument bytes on the stack; the hand asm passes in registers. This is part of the gap on the small ops (ftoi, ftou).
- Not measured: llrm-cc on the same C; -O1.
