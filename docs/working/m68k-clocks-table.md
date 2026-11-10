# m68k executed clocks and bytes: ours against the best of gcc and clang (2026-11-11 00:20Z, m68k-b)

TL;DR: 1112 cells (139 tests/run/c programs x 68000, 68020 x O1 O2 O3 Os), restated on release/next def162091880 (includes #8930 shift pricing and #8942 68020 table forms). 999 cells have clocks: ours better 481, equal 40, worse than the best external 478; 113 are UNMEASURED. With bytes: 402 of the 478 clock-worse cells stay WORSE, 75 ok, 1 VIRTUAL-OK; 107 more are WORSE on bytes with clocks better or equal.

Replaces the 11-10 cards. Every 68020 clock figure published before this is a lower bound for code with the forms #8942 added (it priced `trap #0` at 0: 38 traps = 760 clk in bitcount_u32) and is withdrawn as a bar.

Stamps: ours = llrm-cc 5e4d9ece, llrm-llc bd2cf668, llrm-m68k-llc 8648f4ce, built from def162091880549101b706eb3eaf51c8dbb3ac6d; cycles efe6115a, same commit; both CPUs from this one build. gcc 15 `-msoft-float`; clang = pin clang into pin llc; shared external flags. One exe per cell and column, linked with our compiler-rt archive and the harness stubs; `cycles --exe` with CYCLES_CPUS set to the exe's CPU. Each exe runs under qemu and its stdout is compared to the reference (CRLF read as LF, nothing else forgiven, #8963); a wrong run has no number. Bytes: code+data of the object by the matrix method; library bytes are not included.

Positive control: bitcount_u32 68020 -O2, gcc column (same gcc object both times): 26398 before, 27158 now = +760 = 38 `trap #0` x 20 clk, the 68020 trap cost.

## Clocks only (cells; better / equal / worse / UNMEASURED ours / UNMEASURED no external)

| cpu level | better | equal | worse | ours | no ext |
|---|---|---|---|---|---|
| 68000 O1 | 64 | 2 | 60 | 7 | 6 |
| 68000 O2 | 63 | 7 | 57 | 8 | 4 |
| 68000 O3 | 55 | 7 | 63 | 7 | 7 |
| 68000 Os | 60 | 12 | 54 | 7 | 6 |
| 68020 O1 | 63 | 3 | 58 | 7 | 8 |
| 68020 O2 | 71 | 3 | 52 | 7 | 6 |
| 68020 O3 | 63 | 4 | 56 | 7 | 9 |
| 68020 Os | 42 | 2 | 78 | 7 | 10 |

285 of the 999 measured cells have one external number only (gcc or clang failed, was wrong, or is unpriced): their verdict is against that one, column `externals_with_clocks` = 1 in the tsv. Treat them as partial.

## Clocks and bytes, the 999 measured cells (verdict per level)

| cpu level | WORSE | ok | VIRTUAL-OK |
|---|---|---|---|
| 68000 O1 | 71 | 48 | 7 |
| 68000 O2 | 58 | 69 | 0 |
| 68000 O3 | 66 | 59 | 0 |
| 68000 Os | 51 | 75 | 0 |
| 68020 O1 | 77 | 32 | 15 |
| 68020 O2 | 61 | 65 | 0 |
| 68020 O3 | 65 | 58 | 0 |
| 68020 Os | 60 | 62 | 0 |

The 113 UNMEASURED cells are not in these tables. Their byte-only verdicts in the tsv (65 ok, 16 WORSE, 32 n/a) are not passes.

## The 113 UNMEASURED cells, by cause

| cells | column | cause |
|---|---|---|
| 44 | ours | timing-table gap: indexed forms `(d8,An,Xn)` and full-format `(bd,,Xn)` the pricer rejects (below) |
| 13 | ours | cycles timeout or cut trace (urem_eq, memfuncs_runtime, shl_of_a_zero_extended_dividend) |
| 16 | no external | gcc and clang both failed to compile |
| 12 | no external | gcc long double ABI mismatch, clang wrong output |
| 10 | no external | table gap in one, wrong output in the other |
| 8 | no external | gcc flags with no gcc meaning, clang wrong output |
| 7 | no external | compile failure and a table gap |
| 3 | no external | table gap in both |

os_console (8 ours cells) was a sweep fault, not a miscompile: the program writes CRLF, its .out has LF (#8959, #8963). Now measured: ours 226 clk, clang 271 (68020, all levels); gcc fails to compile it.

## What moved (68020)

`m68k-clocks-moved-68020-2026-11-11.tsv`: every 68020 exe whose figure or state changed, before (c98e231ab1 compiler, pre-#8930/#8942 pricer) and after (def16209). 1520 rows: 1364 went from "unpriced forms" (the trap and the other forms counted as 0) to priced; 8 from wrong-output to priced (os_console); 88 are still unpriced. Deltas mix the compiler change between the two commits and the pricer: ours Os moved most (urem_eq -86%, loop_interchange -74%, recdeep -70%), while the gcc column, whose objects did not change, moved by the pricer alone: all 468 priced exes, median +2.2%, up to +128%.

Of the 28 ours cells that were UNMEASURED at 68020 before, 12 are measured now and 16 remain (timeouts, the forms below).

## Remaining timing-table gaps (exes = cell x column; the pricer prints the operand as shown)

| form | 68000 | 68020 |
|---|---|---|
| `(D,%aN,%dM)`, brief-format index, displacement printed sign-extended (`(ffffffffffffffff,%a3,%d1)`, `(7e,%a1,%d1)`), no size suffix | 84 | 103 |
| `(D,,%dN)`, full-format with the base suppressed | 0 | 53 |

#8942 priced the full-format index with a base register; these are the forms that remain. Owner: the timing session.

## Largest gap

float_in_memory_loop: ours 3.54 M clocks (68000 O2), clang 1798, gcc 5128. Both externals fold the loop (16 x 0.5 added in memory, 100 times); ours runs 1600 soft-float adds. No cause card, no owner.
