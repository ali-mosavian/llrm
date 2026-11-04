# Softfloat C through gcc vs hand Z80 asm, 2026-11-04

TL;DR: gcc is 6.8-31.5x slower than `config/z80/*.s` per op and 7.4x larger (18342 B vs 2491 B, smallest cell); every gcc result is bit-exact. Cells: `softfloat-z80.tsv`.

Method: `runtime/compiler-rt` (`lib/fp/f32/*.c`, `lib/int/int32.c`; `sf32_6502` = `config/6502/sf32_6502.c`, one translation unit per operation), `-DSF_INTABI`, -O2/-Os,
spike gcc Z80 backend (arguments on the stack, result in e d l h; a driver pushes them like gcc), against `config/z80/{sf32_arith,sf32_cmp,int32}.s` through `tests/z80/drv.s`.
Built 2026-11-03 21:54 CET, cc1 md5 59a62e3448b1922f75019a3282b08e38. Hand cells carried from 11-03 (the .s files are unchanged since the move).
Same records, ucsim_z80; clk = (run - bare-`ret` stub run) / records; gcc: 0 fails in all 32 cells.
Records with a NaN input are left out of add/sub/mul/div/ftoi/ftou on both sides: the C propagates the payload, the hand asm returns the canonical NaN
(a harness difference, not a bug: the failing add record passes when the payload is expected).

## Clocks per call (best gcc cell over variant and level)

| op | hand | gcc | gcc/hand | gcc cell |
|---|---|---|---|---|
| add | 1537 | 10686 | 7.0x | -Os sf32_6502 |
| sub | 1576 | 10681 | 6.8x | -Os sf32_6502 |
| mul | 2774 | 87251 | 31.5x | -O2 sf32_6502 |
| div | 3585 | 44602 | 12.4x | -O2 sf32 |
| itof | 658 | 9953 | 15.1x | -Os sf32_6502 |
| utof | 531 | 7640 | 14.4x | -O2 sf32_6502 |
| ftoi | 274 | 3213 | 11.7x | -O2 sf32_6502 |
| ftou | 245 | 2451 | 10.0x | -O2 sf32_6502 |

## Library bytes (_CODE, helpers included)

| | hand | gcc -Os | gcc -O2 |
|---|---|---|---|
| sf32 generic | 2491 | 18805 | 26352 |
| sf32_6502 variant | - | 18342 | 28208 |

## Limits

- The Z80 backend has no native 32-bit add/sub/logic/compare patterns: gcc expands them into byte operations, and 32-bit shifts are library calls
  (`__ashlsi3`, `__lshrsi3`; hand-written asm). `__mulsi3` in these cells is the gcc-compiled one from `int32.c`. The gcc column is an upper bound.
- Every call pushes and pops its 4 or 8 argument bytes on the stack; the hand asm passes in registers. This is part of the gap on the small ops (ftoi, ftou).
- Not measured: llrm-cc on the same C; -O1.
