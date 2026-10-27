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

## In clocks and taken branches (2026-10-22)

Instruction totals say little about these passes, so the same IR was run: `tools/pairs/bench_pin.py` takes our IR (the `switchconv` stage of each wrapped vsgcc program, -mabi=sysv) to our back end (OMF) and to the pin's `llc -O2 -filetype=obj` with and without the switch (ELF, linked as the gcc variants), and runs all three in the 486 model of `crates/target/llrm-x86-m32/vsgcc/harness.py`. 70 of 71 programs (x_strlen does not build); all three variants report the same values.

| switch, level | clocks: ours / pin / pin without | instructions | taken branches | `jmp` executed |
|---|---|---|---|---|
| early taildup, -O2 | 28023372 / 32191215 / 32193646 | 9019311 / 8400560 / 8401472 | 798165 / 786616 / 787175 | 112791 / 94444 / 94567 |
| early taildup, -O3 | 27548222 / 31645466 / 31647899 | 8553035 / 7969515 / 7970427 | 733888 / 726139 / 726699 | 49841 / 54769 / 54892 |
| taildup, -O2 and -O3 | as the pin | as the pin | as the pin | as the pin |
| placement, -O2 | 28023372 / 32191215 / 32461539 | 9019311 / 8400560 / 8408140 | 798165 / 786616 / 917988 | 112791 / 94444 / 102024 |
| placement, -O3 | 27548222 / 31645466 / 31829079 | 8553035 / 7969515 / 7986388 | 733888 / 726139 / 809509 | 49841 / 54769 / 71642 |

The pin's clocks on our IR are above ours: read the columns across for one pass, not the first against the second.

- Early tail duplication is worth 2431 clocks of 32.2 M (0.008%) and 559 taken branches; late tail duplication nothing, on all 70 programs. Rows 34 and 83 are DEVIATE CG3 and CG4, owner w-3, review 2026-11-20, to be re-measured with the `ret` duplication of STRISTMP-1 (main duplicates `ret` into both arms, ours does not: a case this total does not show).
- Placement is worth 0.84% of the pin's clocks at -O2 (0.58% at -O3) and 17% (11%) of its taken branches. Ours has 1.5% (1.1%) more taken branches than the pin and 19% more executed `jmp` at -O2 (9% fewer at -O3). Row 90 is DEVIATE CG5. Programs where ours takes the most extra branches at -O2, for w-2's attribution (taken ours / pin / pin without): nbody_single 16012 / 12002 / 11999, rectwo 20542 / 17207 / 17872, queens 13398 / 10366 / 17377, x_adler 5998 / 3008 / 4478, x_ll_div 4882 / 3431 / 3432, switch 5924 / 4496 / 7288, x_crc32 3154 / 2254 / 2254.

## emit-lowered-select, measured (2026-10-22)

x_adler's `x >= 65521 ? x - 65521 : x` takes a branch every trip in ours and not in the pin. Two causes, both measured:

- the pin's compare is the subtract's flags (`EmitCmp` uses SUB so the DAG shares them; `sub; jae`), landed as `cmpsub@1` (#3773);
- the select diamond oriented as `EmitLoweredSelect` has it (true value on the direct edge, the false value through the empty block): with `cmpsub` the loop is the pin's, but across the corpus it is 75 of 568 objects and +3509 B, clocks +10.5 K of 28.28 M. Without `cmpsub`, +3632 B. Not landed; `selects::_branched` keeps the empty block on the true edge. Owner w-3, review 2026-11-20, with the allocator switch-over.
