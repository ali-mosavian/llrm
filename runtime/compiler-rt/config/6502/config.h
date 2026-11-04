/* The 6502 configuration. The shift libcall (llvm-mos runtime) loops one bit per iteration, ~530 cycles for an average shift, so the addition shifts in byte
 * then bit steps: f32 add through llrm-cc -O2 243984 -> 198690 cycles over 64 operands. The steps cost 18 B on the add path, so -Os (bytes strict) keeps
 * the library default. Include with -include. */
#ifndef SF_6502_H
#define SF_6502_H
/* Rounding by compare: the 32-bit add of the add-3 form costs more than the compare chain here (add -Os +6 B, +1.9% cycles). */
#define SF_ROUND_BRANCHY 1
#define SF_FIX_NAN_UNDEF 1 /* NaN to int is undefined in C and poison in IR: -Os fix_si 230 -> 179 asm lines, fix_usi 176 -> 122 */
#ifndef __OPTIMIZE_SIZE__
#define SF_SHIFT_STEPS 1
#endif
/* mul and div come from config/6502/sf32_6502.c (add stays generic: its tight form is 415 B larger). */
#define SF_EXT_MULDIV 1
#endif
