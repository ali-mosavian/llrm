# Softfloat C through gcc vs hand 6502 asm, 2026-11-03

TL;DR: gcc is 3.0-6.9x slower than `src/6502/sf32*.s` per op and 2.1x larger as a library (-Os); every gcc result is bit-exact. Cells: `softfloat-6502.tsv` (fork SHA and cc1 md5 in its header).

Method: `runtime/softfloat/src/sf32*.c` + `int32.c` (`sf32` generic, `sf32_6502` 6502-shaped), `-DSF_INTABI`, -O2/-Os, against `sf32.s`, `sf32_muldiv.s`, `sf32_conv.s`.
Same 256 vectors of `tests/asm6502/gen.py` on sim65; clk = (run - empty stub) / 256. gcc: 0 fails on 2000 random + 1500 edge vectors per op, both variants, both levels.

## Clocks per call (best gcc cell over variant and level)

| op | hand | gcc | gcc/hand | gcc cell |
|---|---|---|---|---|
| add | 570 | 1719 | 3.0x | -Os sf32 |
| sub | 593 | 1895 | 3.2x | -Os sf32 |
| mul | 1251 | 8635 | 6.9x | -O2 sf32_6502 |
| div | 1407 | 4675 | 3.3x | -O2 sf32_6502 |
| itof | 207 | 750 | 3.6x | -O2 sf32_6502 |
| utof | 171 | 649 | 3.8x | -O2 sf32_6502 |
| ftoi | 150 | 667 | 4.4x | -O2 sf32 |
| ftou | 68 | 442 | 6.5x | -O2 sf32 |

## Library bytes (CODE+RODATA)

| | hand | gcc -Os | gcc -O2 |
|---|---|---|---|
| sf32 arithmetic (+ compares) / hand sf32.s + muldiv | 832 + 1641 | 12393 | 18964 |
| conversions | 1218 | 3047 | 4620 |
| sf32_6502, all members | - | 7768 | 13507 |
| total | 3691 | 7768 (2.1x) | 13507 |

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
- ftoi/ftou: with the native ops off they were 676/400 clk and 6023 B (O2, generic); on, 683/458 clk and 4706 B. No single op group
  is responsible (ftou with add off 462, sub off 451, neg/not off 441). One general cause was fixed (a constant mask emitted `and #255`/`and #0`
  for identity bytes: ftou 458 -> 442); the rest is 4-byte carry chains and copies for zero-extended operands.
- gcc returns in rc2..rc5, the hand code in registers; the caller's move of the result is not charged to either side.
- Not measured: Z80 (its SImode shift/multiply patterns work now, tested on ucsim; `sf32.c` and `sf32_conv.c` compile, `int32.c` still hits the reload limit), llrm-cc on the same C. All -O1 cells pass (0 fails); -O1 clocks are not benched.
