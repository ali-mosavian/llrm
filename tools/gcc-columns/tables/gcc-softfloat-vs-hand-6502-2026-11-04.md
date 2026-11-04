# Softfloat C through gcc vs hand 6502 asm, 2026-11-04

TL;DR: gcc is 2.9-8.2x slower than `config/6502/sf32*.s` per op; every gcc result is bit-exact. The 2026-11-03 table (binary-op clocks 1.4-1.8x too high) is withdrawn. Cells: `softfloat-6502.tsv` (cc1 md5 in its header).

Method: `runtime/compiler-rt` (`lib/fp/f32/*.c`, `lib/int/int32.c`; `sf32_6502` = `config/6502/config.h` knobs with mul/div from `config/6502/sf32_6502.c`), `-DSF_INTABI`, -O2/-Os, cc1 md5 4c91eb80d199e61465970fc620d1b8ea,
against `sf32.s`, `sf32_muldiv.s`, `sf32_conv.s`. Records of `tests/asm6502/gen.py` on sim65: 361 for add/sub/mul/div, 256 for conversions.
clk = (run - base run) / records; the base run pushes the same number of zero records through a stub that returns 0, so it passes them. gcc: 0 fails in all 32 cells.
`sf32_6502` ftoi/ftou drop the 4 NaN-input records (NaN to int is undefined there, `SF_FIX_NAN_UNDEF`). Hand cells re-measured with the same method (`sf-6502/hand.sh`).

## What was wrong in 2026-11-03

It divided by 256 although add/sub/mul/div ran 361 records, and its base run was a stub that failed its records (the driver's report path is not in the real run).
Both columns were inflated alike (hand add 570 now 324, mul 1251 now 807; gcc generic -Os add 1719 now 977), so the ratios mostly held; hand conversion cells (256 records) moved by under 10 clk.
The `sf32_6502` conversions are no longer 6502-shaped: compiler-rt keeps only mul/div in `sf32_6502.c`, so itof/utof run the generic `float_si.c` and are 2x slower than the old variant's.

## Clocks per call (best gcc cell over variant and level)

| op | hand | gcc | gcc/hand | gcc cell |
|---|---|---|---|---|
| add | 324 | 941 | 2.9x | -O2 sf32_6502 |
| sub | 340 | 1043 | 3.1x | -O2 sf32_6502 |
| mul | 807 | 6023 | 7.5x | -O2 sf32_6502 |
| div | 918 | 3216 | 3.5x | -O2 sf32_6502 |
| itof | 213 | 1683 | 7.9x | -O2 sf32_6502 |
| utof | 177 | 1458 | 8.2x | -O2 sf32_6502 |
| ftoi | 156 | 512 | 3.3x | -Os sf32_6502 |
| ftou | 74 | 244 | 3.3x | -Os sf32 |

## Bytes (CODE+RODATA+DATA of the members one op links, hand helpers included)

Hand: `sf32.s` 832 + `sf32_muldiv.s` 809 + `sf32_conv.s` 386 = 2027 B for all ops (the 11-03 table added `sf32.s` into the conversions twice).
gcc per op: `softfloat-6502.tsv`. At -Os, sf32_6502: add 2888, mul 3700, itof 1236, ftoi 535 B.
