# Softfloat C through gcc vs hand Z80 asm, 2026-11-04

TL;DR: with native 32-bit add/sub/move/compare gcc is 5.9-29.1x slower than `config/z80/*.s` per op and 5.1x larger (12796 B vs 2491 B, smallest cell); every gcc result is bit-exact. Cells: `softfloat-z80.tsv`.

Method: `runtime/compiler-rt` (`lib/fp/f32/*.c`, `lib/int/int32.c`; `sf32_6502` = `config/6502/sf32_6502.c`, one translation unit per operation), `-DSF_INTABI`, -O2/-Os,
spike gcc Z80 backend (arguments on the stack, result in e d l h; a driver pushes them like gcc), against `config/z80/{sf32_arith,sf32_cmp,int32}.s` through `tests/z80/drv.s`.
Built 2026-11-04 01:27 CET, cc1 md5 38cd2b7db804f89e26de9aecd70fe441 (fork cd4fbdff8). Hand cells carried from 11-03 (the .s files are unchanged since the move).
Same records, ucsim_z80; clk = (run - bare-`ret` stub run) / records; gcc: 0 fails in all 32 cells.
Records with a NaN input are left out of add/sub/mul/div/ftoi/ftou on both sides: the C propagates the payload, the hand asm returns the canonical NaN
(a harness difference, not a bug: the failing add record passes when the payload is expected).

## Clocks per call (best gcc cell over variant and level)

| op | hand | gcc | gcc/hand | gcc cell |
|---|---|---|---|---|
| add | 1537 | 9256 | 6.0x | -Os sf32_6502 |
| sub | 1576 | 9266 | 5.9x | -Os sf32_6502 |
| mul | 2774 | 80799 | 29.1x | -O2 sf32_6502 |
| div | 3585 | 32123 | 9.0x | -O2 sf32 |
| itof | 658 | 9244 | 14.0x | -Os sf32_6502 |
| utof | 531 | 6788 | 12.8x | -Os sf32_6502 |
| ftoi | 274 | 2994 | 10.9x | -O2 sf32 |
| ftou | 245 | 2368 | 9.7x | -O2 sf32 |

## Library bytes (_CODE, helpers included)

| | hand | gcc -Os | gcc -O2 |
|---|---|---|---|
| sf32 generic | 2491 | 13057 | 19848 |
| sf32_6502 variant | - | 12796 | 21029 |

Against the previous cc1 (59a62e34, longs split into bytes): clocks 0.68-0.99x (geomean 0.88), library bytes -25..-30%, no cell worse; the 30 kernels are identical.

## Limits

- 32-bit logic and shifts are still byte operations or library calls (native: add, sub, move, compare)
  (`__ashlsi3`, `__lshrsi3`; hand-written asm). `__mulsi3` in these cells is the gcc-compiled one from `int32.c`. The gcc column is an upper bound.
- Every call pushes and pops its 4 or 8 argument bytes on the stack; the hand asm passes in registers. This is part of the gap on the small ops (ftoi, ftou).
- Not measured: llrm-cc on the same C; -O1.
