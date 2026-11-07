/* The 6502 configuration. The shift libcall (llvm-mos runtime) loops one bit per iteration, ~530 cycles for an average shift, so the addition shifts in byte
 * then bit steps: f32 add through llrm-cc -O2 243984 -> 198690 cycles over 64 operands. The steps cost 18 B on the add path, so -Os (bytes strict) keeps
 * the library default. Include with -include. */
#ifndef SF_6502_H
#define SF_6502_H
/* Rounding by compare: the 32-bit add of the add-3 form costs more than the compare chain here (add -Os +6 B, +1.9% cycles). */
#define SF_ROUND_BRANCHY 1
#define SF_FIX_NAN_UNDEF 1 /* NaN to int is undefined in C and poison in IR: -Os fix_si 230 -> 179 asm lines, fix_usi 176 -> 122 */
#define SF_SHIFT_LOOP 1 /* shift by a per-bit loop: fix_si keeps the two-direction shift (single larger count: 6502 -Os +15% cycles) */
#ifndef __OPTIMIZE_SIZE__
#define SF_SHIFT_STEPS 1
#else
#define SF_DIV64_STEP_SHIFT 1 /* the 64-bit divide shifts its divisor by byte and bit steps, not shift libcalls: int64_division -Os 5601 -> 5283 B, +0.3% clocks (-O2 +16% clocks) */
#endif
/* mul and div come from config/6502/sf32_6502.c (add stays generic: its tight form is 415 B larger). */
#define SF_EXT_MULDIV 1
#define SF_MUL32X32_6502 1 /* f64 mul's 32x32 -> 64 is config/6502/mul32x32.s (hand asm, link it; static cells, not reentrant): -Os 41312 -> 11504 clk, 5592 -> 5115 B (llvm-mos 18270 / 5509) */
#endif
