# M68K-BITCOUNT: whole-program bytes against the gcc-style call form (68020)

Owner: m68k-68020 (cells below carry their own owner and exit).

TL;DR: with each column linking only the library members it references (`tools/m68k_program_size.py`, compiler-rt in both columns), ours is smaller at 68000 and on `bitcount_u64`, and larger on `bitcount_u32` at 68020. The open cells are listed, not hidden by the u64 win.

| program | CPU | level | ours program + library = whole | gcc-style program + library = whole | delta |
|---|---|---|---|---|---|
| bitcount_u64 | 68020 | Os | 594 + 0 = 594 | 390 + 248 = 638 | -44 |
| bitcount_u64 | 68020 | O2 | 614 + 0 = 614 | 388 + 248 = 636 | -22 |
| bitcount_u32 | 68020 | Os | 330 + 0 = 330 | 254 + 70 = 324 | +6 OPEN |
| bitcount_u32 | 68020 | O2 | 358 + 0 = 358 | 254 + 70 = 324 | +34 OPEN |
| bitcount_u64 | 68000 | Os, O2 | 370 + 292 = 662 | 382 + 292 = 674 | -12 |
| bitcount_u32 | 68000 | Os, O2 | 248 + 176 = 424 | 264 + 176 = 440 | -16 |

`.text` bytes of the program object and of the archive members pulled (ld map), -Os routines at the same level, llrm-cc built from release/next `5087459800` plus the `foldarg` bfffo fold (#8455). The gcc column is gcc's code with our compiler-rt (libgcc's m68k routines are not in the gcc root), so its library bytes are an estimate.

## Open cells
| cell | delta | cause | owner | exit |
|---|---|---|---|---|
| bitcount_u32 68020 Os (strict) | +6 B whole (330 vs 324) | `lead0` 18 vs 8: `x ? clz(x) : 32` keeps a branch (pin InstCombine `foldSelectCttzCtlz`; w10-instcombine, draft 2026-11-11); `lead` 10 vs 8; inline popcount wins 22 B | m68k-68020 | 2026-11-16 |
| bitcount_u32 68020 O2 | +34 B whole (358 vs 324) | the same, plus `main` 28 B larger than at Os (gcc's is the same at both); cause traced by 2026-11-11 | m68k-68020 | 2026-11-16 |
| bitcount_u64 68020 Os, O2 (after #8464) | +56 B / +78 B whole (594 vs 538, 614 vs 536) | the `bfffo` runtime (#8464) makes the call form cheap: ctz64 inline 50 B vs 18 + `__ctzdi2` 46; popcount64 124 B vs 18 + 102 | m68k-68020 | 2026-11-12 (the per-site inline-vs-call PR) |
| BC1 note in `dagisel/lowering.rs` (#8383, "u64 O2: -540 B, +4000 clk") | to re-measure with `m68k_program_size.py` | measured with every routine linked into both columns | m68k-68020 | 2026-11-12 |

The wins stay listed above: before #8464 the u64 rows were -44 / -22 B.

The 68020 compiler-rt has `bfffo` routines from #8464; the inline-vs-call choice is then per site by size and speed cost (the two rules: size <= call cost, or a loop hot path).

## Relabelled figures
Earlier byte figures from `m68k-bitcount-ab-2026-11-08/prog.sh` (`text=` of an executable) linked all six count routines into BOTH columns, so a libcall column showed no library cost: bitcount_u32 828/876 vs 772 and bitcount_u64 1168/1148 vs 940/944 in the PR #8432 body are program-object-dominated, not whole-program. The BC1 note in `dagisel/lowering.rs` ("u64 O2: -540 B, +4000 clk", #8383) is to be re-measured with the new tool.
