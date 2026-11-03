# Softfloat C through gcc vs hand 6502 asm, 2026-11-03

TL;DR: gcc is 3.1-7.5x slower than `src/6502/sf32*.s` per op and 2.2x larger as a library (-Os); every gcc result is bit-exact. Cells: `softfloat-6502.tsv` (fork SHA and cc1 md5 in its header).

Method: `runtime/softfloat/src/sf32*.c` + `int32.c` (`sf32` generic, `sf32_6502` 6502-shaped), `-DSF_INTABI`, -O2/-Os, against `sf32.s`, `sf32_muldiv.s`, `sf32_conv.s`.
Same 256 vectors of `tests/asm6502/gen.py` on sim65; clk = (run - empty stub) / 256. gcc: 0 fails on 2000 random + 1500 edge vectors per op, both variants, both levels.

## Clocks per call (best gcc cell over variant and level)

| op | hand | gcc | gcc/hand | gcc cell |
|---|---|---|---|---|
| add | 570 | 1766 | 3.1x | -Os sf32 |
| sub | 593 | 1938 | 3.3x | -Os sf32 |
| mul | 1251 | 9354 | 7.5x | -O2 sf32_6502 |
| div | 1407 | 4786 | 3.4x | -O2 sf32_6502 |
| itof | 207 | 752 | 3.6x | -O2 sf32_6502 |
| utof | 171 | 651 | 3.8x | -O2 sf32_6502 |
| ftoi | 150 | 683 | 4.6x | -O2 sf32 |
| ftou | 68 | 458 | 6.7x | -O2 sf32 |

## Library bytes (CODE+RODATA)

| | hand | gcc -Os | gcc -O2 |
|---|---|---|---|
| sf32 arithmetic (+ compares) / hand sf32.s + muldiv | 832 + 1641 | 12723 | 19314 |
| conversions | 1218 | 3103 | 4706 |
| sf32_6502, all members | - | 8068 | 13887 |
| total | 3691 | 8068 (2.2x) | 13887 |

Coverage differs: the gcc objects also hold the compares and negate, which the hand files do not. The per-op byte split is not measured.

## Causes (read from the asm)

1. mul is the widest gap: the 24-step shift-add loop runs on 32-bit zero-page values, so each step is byte-wise `lda/op/sta` chains plus the carry
   bookkeeping, where the hand code keeps the live byte in A/X/Y and shifts in place.
2. Every 32-bit value lives in zero-page bytes; calls copy 4+4 argument bytes, and a third argument goes through the soft stack.
3. Variable shifts are one-bit-per-trip library loops; the hand code moves whole bytes, then at most 7 bit steps.
4. At -O2 each of the six compare entry points inlines the compare core: -O2 is larger than -Os in bytes.

## Limits

- Native 32-bit add/sub/logic/neg/not/compare/shift-by-constant are on. An earlier run with them off, and an apparent "miscompile" when they were on, were
  one backend bug: the frame did not reserve the outgoing-argument area, so a stack-passed argument overwrote a local (fixed; test `stackarg.c`).
  Before the fix, best mul was 13485 and div 6509 clk, the -Os library 9394 B.
- gcc returns in rc2..rc5, the hand code in registers; the caller's move of the result is not charged to either side.
- Not measured: Z80 (backend ICEs on SImode shift/multiply), llrm-cc on the same C. All -O1 cells pass (0 fails); -O1 clocks are not benched.
