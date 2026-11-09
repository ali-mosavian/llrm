# M68K-BITCOUNT: whole-program bytes against the gcc-style call form (68020)

Owner: m68k. Exit: 2026-11-20.

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

## Open cells (68020 `bitcount_u32`)
- Per function at -Os, ours vs gcc: `lead` 10 vs 8, `lead0` 18 vs 8 (`x ? clz(x) : 32` keeps a branch: pin InstCombine `foldSelectCttzCtlz`, w10-instcombine), `ones` inline 62 vs a 14 B call (+70 B library, so the inline form wins by 22 B whole-program).
- O2 adds 28 B in `main` over -Os (gcc's `main` is the same at both); cause not traced yet.
- The 68020 compiler-rt has no `bfffo` variant (`__clzsi2` is a 76 B ladder); with one, the call form gets cheaper and the inline-vs-call choice is per site (size + speed cost).

## Relabelled figures
Earlier byte figures from `m68k-bitcount-ab-2026-11-08/prog.sh` (`text=` of an executable) linked all six count routines into BOTH columns, so a libcall column showed no library cost: bitcount_u32 828/876 vs 772 and bitcount_u64 1168/1148 vs 940/944 in the PR #8432 body are program-object-dominated, not whole-program. The BC1 note in `dagisel/lowering.rs` ("u64 O2: -540 B, +4000 clk", #8383) is to be re-measured with the new tool.
