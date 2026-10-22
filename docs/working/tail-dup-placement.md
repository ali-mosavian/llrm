# Tail duplication and block placement: premise tests — w-3, 2026-10-22

Rows 34 (early tail duplication), 83 (tail duplication), 90 (branch-probability block placement) of [codegen-slot-table.md](codegen-slot-table.md). Test: `tools/pairs/pinswitch.py`, our IR at the `switchconv` stage to our back end and to the pin's `llc -O2` with and without the pass's switch; 71 bench programs at m32, 65 QCport modules at m16. Static counts: all instructions, instructions inside loops (a label to a later branch back to it), jumps (`jmp` and `jcc`).

| pass (switch), corpus | all: ours / pin / pin without | in loops: ours / pin / without | jumps: ours / pin / without |
|---|---|---|---|
| early taildup, m32 -O2 | 9813 / 8413 / 8416 | 4904 / 4182 / 4186 | 729 / 698 / 699 |
| early taildup, m32 -Os | 5255 / 5047 / 5047 | 2612 / 2662 / 2663 | 437 / 450 / 450 |
| early taildup, m16 -O2 | 93207 / 88072 / 88103 | 49715 / 46865 / 46898 | 9171 / 9193 / 9203 |
| early taildup, m16 -Os | 76516 / 74693 / 74694 | 38291 / 36701 / 36708 | 6222 / 6483 / 6485 |
| taildup, m32 -O2 | = pin | = pin | = pin (no effect) |
| taildup, m32 -Os | = pin | = pin | = pin (no effect) |
| taildup, m16 -O2 | 93207 / 88072 / 88033 | 49715 / 46865 / 46645 | 9171 / 9193 / 9184 |
| taildup, m16 -Os | = pin | = pin | = pin (no effect) |
| placement, m32 -O2 | 9813 / 8413 / 8398 | 4904 / 4182 / 5402 | 729 / 698 / 677 |
| placement, m32 -Os | 5255 / 5047 / 5099 | 2612 / 2662 / 3239 | 437 / 450 / 502 |
| placement, m16 -O2 | 93207 / 88072 / 88580 | 49715 / 46865 / 65146 | 9171 / 9193 / 9613 |
| placement, m16 -Os | 76516 / 74693 / 75573 | 38291 / 36701 / 54100 | 6222 / 6483 / 7334 |

## Reading

- **Tail duplication (rows 34, 83): the premise fails.** The passes move at most 0.04% of the pin's instructions (3 to 39) and at most 9 jumps; the one visible effect is late tail duplication at m16 -O2, 220 fewer in-loop instructions of 46865 (0.47%). Nothing to translate for size or static loops.
- **Block placement (row 90): jumps are at the pin's level, loop layout is not at -O2.** Switching the pass off costs the pin 420 jumps at m16 -O2 and 851 at m16 -Os; ours has 9171 against the pin's 9193 and 6222 against 6483, so we keep the jumps the pass saves. The in-loop range, which grows when cold blocks stay inside a loop, shows the rest: ours closes 91% of the pass's effect at m16 -Os and 85% at m16 -O2, but at m32 -O2 only 59% (4904 against the pin's 4182 and 5402 without). m32 -Os is better than the pin (2612 against 2662).
- The in-loop figure also carries differences that are not layout (ours has 1400 more instructions at m32 -O2), so the m32 -O2 gap of 722 is an upper bound.

## Next

No code for rows 34 and 83. For row 90, attribute the m32 -O2 in-loop excess per function (which loops keep a cold block inside) before any change; the rows' owner is lir-cleanup-port. Limits: static counts, one IR stage, no run times; the loop range is a proxy for the pin's block frequency.
