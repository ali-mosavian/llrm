# M68K-STACK-ALIGN: the frame rounds to 4, not the pin's 8

Owner: m68k. Review: 2026-12-20.

TL;DR: `SHIPPED_STACK_ALIGN = 4` (`frame.rs`) against the pin's `stackAlignment = 8` (`M68kSubtarget.h:69`) is worth 0.5-0.8% of code bytes at O2/Os. Kept: the m68k Linux, AmigaOS and TOS ABIs ask for 4, and the pin's 8 only pads.

## Price of sp kept 8-aligned (executed A/B, bytes of `.text`, 2026-11-09)
Same tree, one constant changed (`SHIPPED_STACK_ALIGN` 4 to 8); `bench` + `tests/run/c` programs, ours only (`ab_sweep.py`).

| cpu | level | cells | base bytes | +bytes with 8 | cells larger / smaller |
|---|---|---|---|---|---|
| 68000 | O2 | 175 | 307593 | +1504 (0.49%) | 88 / 18 |
| 68000 | Os | 176 | 292951 | +2038 (0.70%) | 101 / 0 |
| 68020 | O2 | 175 | 302169 | +2402 (0.79%) | 83 / 6 |
| 68020 | Os | 176 | 286413 | +1692 (0.59%) | 91 / 0 |

Clocks were not measured; the pad is an `lea`/`addq` pair per call-making function.

## Consequence
sp is 4 mod 8 at some calls, so a callee that rounds a pointer to 8 reads another address there. The one place it showed is `va_arg` of a `double`: fixed callee-side (`VA-ALIGN`, row M68K-VA-DOUBLE), so the frame stays at 4.
