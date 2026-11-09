# M68K-BITCOUNT: whole-program bytes against the gcc-style call form (68020)

Owner: m68k-68020 (cells below carry their own owner and exit).

TL;DR: with each column linking only the library members it references (`tools/m68k_program_size.py`, compiler-rt in both columns), ours is smaller at 68000 and larger at 68020 on `bitcount_u32` (+6/+34 B) and `bitcount_u64` (+36/+78 B after the per-site pass). The gcc library column was 248 B before the `bfffo` runtime (#8464) and is 148 B now; the old u64 "-44" rows were stale.

| program | CPU | level | ours program + library = whole | gcc-style program + library = whole | delta |
|---|---|---|---|---|---|
| bitcount_u64 | 68020 | Os | 426 + 148 = 574 (M68K-SIZE-LIBCALL; inline 594 + 0) | 390 + 148 = 538 | +36 OPEN |
| bitcount_u64 | 68020 | O2 | 614 + 0 = 614 | 388 + 148 = 536 | +78 OPEN |
| bitcount_u32 | 68020 | Os | 330 + 0 = 330 | 254 + 70 = 324 | +6 OPEN |
| bitcount_u32 | 68020 | O2 | 358 + 0 = 358 | 254 + 70 = 324 | +34 OPEN |
| bitcount_u64 | 68000 | Os, O2 | 370 + 292 = 662 | 382 + 292 = 674 | -12 |
| bitcount_u32 | 68000 | Os, O2 | 248 + 176 = 424 | 264 + 176 = 440 | -16 |

`.text` bytes of the program object and of the archive members pulled (ld map), -Os routines at the same level, llrm-cc built from release/next `5087459800` plus the `foldarg` bfffo fold (#8455). The gcc column is gcc's code with our compiler-rt (libgcc's m68k routines are not in the gcc root), so its library bytes are an estimate.

## Open cells
| cell | delta | cause | owner | exit |
|---|---|---|---|---|
| bitcount_u32 68020 Os (strict) | +6 B whole (330 vs 324) | `lead0` 18 vs 8: `x ? clz(x) : 32` keeps a branch (pin InstCombine `foldSelectCttzCtlz`; w10-instcombine, draft 2026-11-11); `lead` 10 vs 8; inline popcount wins 22 B | m68k-68020 | 2026-11-16 |
| bitcount_u32 68020 O2 | +34 B whole (358 vs 324) | the same, plus `main` 28 B larger than at Os: traced 2026-11-09, -Os keeps `report` in `%a3` (`jsr (%a3)`, 2 B x 8) and -O2 calls it absolute (6 B x 8): the call-target-register trade (card m68k-call-target-register-card-2026-11-08), not bit-count code | m68k-68020 | 2026-11-16 |
| bitcount_u64 68020 Os | +36 B whole (574 vs 538) after M68K-SIZE-LIBCALL (594 inline, -20 B) | rest: `x ? clz(x) : 32` keeps its branch (w10 foldSelectCttzCtlz), stack-argument handling; the pass costs +916 clk (+7%) at 68020, +611 (+18%) at 68060 for the 20 B (Os is strict bytes) | m68k-68020 (stack args); w10-instcombine (fold) | 2026-11-16 (stack args); 2026-11-11 (fold, draft) |
| bitcount_u64 68020 O2 | +78 B whole (614 vs 536) | the pass is Os-only: inline wins on clocks at O2 (table below) | m68k-68020 | rate verdict by 2026-11-16 |
| BC1 note in `dagisel/lowering.rs` (#8383, "u64 O2: -540 B, +4000 clk") | re-measured 2026-11-09 (below) | measured with every routine linked into both columns | m68k-68020 | note rewritten with the next lowering PR, 2026-11-12 |

The wins stay listed above: before #8464 the u64 rows were -44 / -22 B.

The 68020 compiler-rt has `bfffo` routines from #8464; the inline-vs-call choice is then per site by size and speed cost (the two rules: size <= call cost, or a loop hot path).

## Relabelled figures
Earlier byte figures from `m68k-bitcount-ab-2026-11-08/prog.sh` (`text=` of an executable) linked all six count routines into BOTH columns, so a libcall column showed no library cost: bitcount_u32 828/876 vs 772 and bitcount_u64 1168/1148 vs 940/944 in the PR #8432 body are program-object-dominated, not whole-program. The BC1 note in `dagisel/lowering.rs` ("u64 O2: -540 B, +4000 clk", #8383) is to be re-measured with the new tool.

## BC1 re-measure at 68020 (2026-11-09, bfffo runtime of #8464, whole linked program)
`ctz64` zero-poison and `popcount` 32/64 as libcalls (an env-gated experiment build, not shipped) against inline; clocks from `cycles --exe`, 68020 / 68030 / 68040 / 68060.

| program | level | bytes inline -> libcall | clk 68020 | clk 68060 |
|---|---|---|---|---|
| bitcount_u32 | Os | 542 -> 566 | 8547 -> 8571 | 2488 -> 2624 |
| bitcount_u32 | O2 | 570 -> 594 | 8526 -> 8550 | 2502 -> 2638 |
| bitcount_u64 | Os | 842 -> 826 | 12582 -> 13498 | 3438 -> 4049 |
| bitcount_u64 | O2 | 862 -> 850 | 12345 -> 13366 | 3424 -> 4053 |
| bitcount_idioms | Os | 652 -> 682 | 16158 -> 16534 | 4224 -> 4360 |
| bitcount_idioms | O2 | 662 -> 694 | 16040 -> 16504 | 4208 -> 4360 |

A blanket libcall loses clocks everywhere (+3% to +18%) and bytes on u32 and idioms; it wins 12 to 16 B on u64 only. Inline stays right at 68020+ at -O2; at -Os only a per-site choice (ctz64 and popcount64 past their break-even use counts) could gain, at most about 20 B in one program.

## M68K-SIZE-LIBCALL (2026-11-09)
`libcalls.tsv` lists each candidate's routine and the routine's linked bytes; the bytes of a site, inline and as a call, are measured per function. A size-built function calls where the module's total is smaller, a member linked once. Per use count, ctz64 (plain site: inline 46 B, call 18 B, member 48 B), run on qemu 68020/40/60 for 1, 2, 3 uses:

| uses | inline B | call B | choice |
|---|---|---|---|
| 1 | 46 | 66 | inline |
| 2 | 92 | 84 | call (-8) |
| 3 | 138 | 102 | call (-36) |

Variable 64-bit shifts, the second class, and the other kinds are priced by the bytes each function builds to with and without the call (M68K-SIZE-LIBCALL-COVER): a site's inline size depends on its operands, so a flat column called `__ashldi3` in shuttle_value (68020 Os 230 -> 286 B, fixed out of the table in #8487). Measured at Os, 136 programs vs before the pass: 68020/68040/68060 135 identical, bitcount_u64 594 -> 574 whole the only moved cell; 68000 all identical. Compile time over the 136 at Os: +5.4% total, geomean +2.9% (load 43). Ops the table takes next (same rows, no new code): 32/64-bit divide and remainder, soft-float add/mul.
