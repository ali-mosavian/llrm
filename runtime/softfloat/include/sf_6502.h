/* The 6502 configuration. The shift libcall (llvm-mos runtime) loops one bit per iteration, ~530 cycles for an average shift, so the addition shifts in byte
 * then bit steps: f32 add through llrm-cc -O2/-Os 262323 -> 220462 cycles over 64 operands. The steps cost 18 B on the add path, so -Os (bytes strict) keeps
 * the library default. Include with -include. */
#ifndef SF_6502_H
#define SF_6502_H
#ifndef __OPTIMIZE_SIZE__
#define SF_SHIFT_STEPS 1
#endif
#endif
